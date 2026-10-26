//! The editor's interface around the viewport: toolbar, scene hierarchy,
//! inspector, viewport overlays, status bar and dialogs.
//!
//! Edits to project data happen inline (through [`Editor::edit`] or, for
//! continuous edits like typing and dragging, by recording undo once when
//! the interaction starts). Actions that need the engine are collected as
//! [`Action`]s and applied after the UI frame is built.

use pulga_engine::gui::{ButtonKind, DrawCommand, Icon, Rect, Ui, UiKey};
use pulga_engine::project::{Item, Order};
use pulga_engine::zone::{Prop, Zone, ZoneKind};
use pulga_engine::{Color, Engine, font};

use super::viewport::{PALETTE, home_camera};
use super::{Confirm, Editor, Selection};
use pulga_engine::runner::Death;

const TOP: f32 = 48.0;
const STATUS: f32 = 28.0;
const LEFT: f32 = 290.0;
const RIGHT: f32 = 340.0;

enum Action {
    Undo,
    Redo,
    Save,
    TogglePreview,
    Play,
    Theme,
    ResetView,
    Focus,
    Duplicate,
    Delete,
    Confirmed(Confirm),
}

impl Editor {
    pub(super) fn draw_panels(&mut self, engine: &mut Engine) {
        let (w, h) = engine.screen_size();
        let mut actions = Vec::new();
        let ui = &mut engine.gui;
        self.toolbar(ui, w, &mut actions);
        self.hierarchy(
            ui,
            Rect::new(0.0, TOP, LEFT, h - TOP - STATUS),
            &mut actions,
        );
        self.inspector(
            ui,
            Rect::new(w - RIGHT, TOP, RIGHT, h - TOP - STATUS),
            &mut actions,
        );
        self.viewport_overlay(ui, Rect::new(LEFT, TOP, w - LEFT - RIGHT, h - TOP - STATUS));
        self.status_bar(ui, Rect::new(0.0, h - STATUS, w, STATUS));
        self.dialogs(ui, &mut actions);
        for action in actions {
            self.apply(engine, action);
        }
    }

    fn apply(&mut self, engine: &mut Engine, action: Action) {
        match action {
            Action::Undo => self.undo(engine),
            Action::Redo => self.redo(engine),
            Action::Save => self.save(Some(engine)),
            Action::TogglePreview if self.preview.is_some() => self.stop_preview(engine),
            Action::TogglePreview => self.start_preview(engine),
            Action::Play => self.play(engine),
            Action::Theme => self.set_theme(engine, !self.dark),
            Action::ResetView => self.camera = home_camera(),
            Action::Focus => self.focus_selection(),
            Action::Duplicate => self.duplicate_selection(engine),
            Action::Delete => self.delete_selection(),
            Action::Confirmed(what) => {
                self.confirm = None;
                self.confirmed(engine, what);
            }
        }
    }

    // --------------------------------------------------------------- toolbar

    fn toolbar(&mut self, ui: &mut Ui, w: f32, actions: &mut Vec<Action>) {
        let previewing = self.preview.is_some();
        let (can_undo, can_redo, dirty) =
            (self.history.can_undo(), self.history.can_redo(), self.dirty);
        let project = self.project.name.clone();
        ui.bar("toolbar", Rect::new(0.0, 0.0, w, TOP), |ui| {
            ui.label_colored("pulga", ui.theme.accent_hover);
            ui.label_colored("editor", ui.theme.text_dim);
            ui.space(14.0);
            ui.badge(&project, ui.theme.text_dim);
            ui.space(18.0);
            if ui
                .icon_button(
                    Icon::Undo,
                    if can_undo {
                        "Undo  Ctrl+Z"
                    } else {
                        "Nothing to undo"
                    },
                    false,
                )
                .clicked
            {
                actions.push(Action::Undo);
            }
            if ui
                .icon_button(
                    Icon::Redo,
                    if can_redo {
                        "Redo  Ctrl+Y"
                    } else {
                        "Nothing to redo"
                    },
                    false,
                )
                .clicked
            {
                actions.push(Action::Redo);
            }
            if ui
                .icon_button(
                    Icon::Save,
                    if dirty {
                        "Save  Ctrl+S  (unsaved changes)"
                    } else {
                        "Save  Ctrl+S"
                    },
                    dirty,
                )
                .clicked
            {
                actions.push(Action::Save);
            }
            ui.space(18.0);
            let preview = if previewing {
                ui.button_with(ButtonKind::Danger, Some(Icon::Stop), "Stop preview")
            } else {
                ui.button_with(ButtonKind::Primary, Some(Icon::Play), "Run preview")
            };
            if preview
                .on_hover_text(ui, "Run through the selection  P")
                .clicked
            {
                actions.push(Action::TogglePreview);
            }
            if ui
                .button_with(ButtonKind::Ghost, Some(Icon::Run), "Play game")
                .on_hover_text(ui, "Build and run the game  F5")
                .clicked
            {
                actions.push(Action::Play);
            }
            ui.space(12.0);
            ui.checkbox(&mut self.snap, "Snap");
        });
        let theme = if self.dark {
            "Light theme"
        } else {
            "Dark theme"
        };
        let right = Rect::new(
            w - 3.0 * (30.0 + 6.0) - 10.0,
            (TOP - 30.0) / 2.0,
            120.0,
            30.0,
        );
        ui.put_horizontal(right, |ui| {
            if ui
                .icon_button(Icon::Focus, "Focus selection  F", false)
                .clicked
            {
                actions.push(Action::Focus);
            }
            if ui.icon_button(Icon::Cube, "Reset view", false).clicked {
                actions.push(Action::ResetView);
            }
            if ui.icon_button(Icon::Theme, theme, false).clicked {
                actions.push(Action::Theme);
            }
        });
    }

    // ------------------------------------------------------------- hierarchy

    fn hierarchy(&mut self, ui: &mut Ui, rect: Rect, actions: &mut Vec<Action>) {
        ui.panel("hierarchy", rect, |ui| {
            ui.columns(2, |ui| {
                ui.label_colored("Scene", ui.theme.text_dim);
                ui.menu_button(Some(Icon::Plus), "New", |ui| {
                    for kind in ZoneKind::ALL {
                        if ui.menu_item(Some(Icon::Cube), &format!("{} zone", kind.name())) {
                            let id = self.edit(|p| p.add_zone(kind.name(), kind));
                            self.select(Selection::Zone(id));
                        }
                    }
                    if ui.menu_item(Some(Icon::Folder), "Group") {
                        let id = self.edit(|p| p.add_group("Group", Vec::new()));
                        self.select(Selection::Group(id));
                    }
                });
            });
            ui.separator();
            let list_h = (ui.available().h - 2.0 * ui.theme.row_height - 3.0 * ui.theme.spacing - 34.0).max(60.0);
            ui.scroll_area("tree", list_h, |ui| {
                for (i, item) in self.project.roots().into_iter().enumerate() {
                    ui.push_id(i, |ui| self.tree_item(ui, item, 0));
                }
                if self.project.zones.is_empty() && self.project.groups.is_empty() {
                    ui.caption("Nothing here yet: add a zone with  + New.");
                }
            });
            ui.separator();
            ui.caption("Zones outside groups are glued at random\nin free mode; a group counts as one zone.");
            let has = self.selection != Selection::Nothing;
            ui.columns(2, |ui| {
                if ui.button_with(ButtonKind::Normal, Some(Icon::Copy), "Duplicate").clicked && has {
                    actions.push(Action::Duplicate);
                }
                if ui.button_with(ButtonKind::Normal, Some(Icon::Trash), "Delete").clicked && has {
                    actions.push(Action::Delete);
                }
            });
        });
    }

    fn tree_item(&mut self, ui: &mut Ui, item: Item, depth: usize) {
        match item {
            Item::Zone(id) => {
                let Some(zone) = self.project.zone(id) else {
                    return;
                };
                let (name, tag) = (zone.name.clone(), zone.kind.tag());
                let selected = self.selection == Selection::Zone(id);
                let badge = Some((tag, ui.theme.accent_hover));
                if ui
                    .list_row(
                        ("zone", id),
                        depth,
                        None,
                        Some(Icon::Cube),
                        &name,
                        selected,
                        badge,
                    )
                    .clicked
                {
                    self.select(Selection::Zone(id));
                }
            }
            Item::Group(id) => {
                let Some(group) = self.project.group(id) else {
                    return;
                };
                let (name, children, order) =
                    (group.name.clone(), group.children.clone(), group.order);
                let selected = self.selection == Selection::Group(id);
                let mut open = *self.open_groups.entry(id).or_insert(true);
                let count = children.len().to_string();
                let icon = if order == Order::Shuffle {
                    Icon::Shuffle
                } else {
                    Icon::Folder
                };
                let badge = Some((count.as_str(), ui.theme.success));
                if ui
                    .list_row(
                        ("group", id),
                        depth,
                        Some(&mut open),
                        Some(icon),
                        &name,
                        selected,
                        badge,
                    )
                    .clicked
                {
                    self.select(Selection::Group(id));
                }
                self.open_groups.insert(id, open);
                if open && depth < 16 {
                    for (i, child) in children.into_iter().enumerate() {
                        ui.push_id(("child", id, i), |ui| self.tree_item(ui, child, depth + 1));
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------- inspector

    fn inspector(&mut self, ui: &mut Ui, rect: Rect, actions: &mut Vec<Action>) {
        ui.panel("inspector", rect, |ui| {
            ui.scroll_area("inspector-scroll", f32::INFINITY, |ui| {
                match self.selection {
                    Selection::Zone(id) => self.zone_inspector(ui, id, actions),
                    Selection::Group(id) => self.group_inspector(ui, id, actions),
                    Selection::Nothing => self.project_inspector(ui),
                }
            });
        });
    }

    /// A text field bound to a project string: undo is recorded once when
    /// editing starts, every keystroke applies.
    fn name_field(
        &mut self,
        ui: &mut Ui,
        key: &str,
        current: String,
        hint: &str,
        apply: impl FnOnce(&mut super::Project, String),
    ) {
        let mut text = current;
        let r = ui.text_input(key, &mut text, hint);
        if r.pressed {
            self.history.record(&self.project);
        }
        if r.changed {
            apply(&mut self.project, text);
            self.dirty = true;
        }
    }

    fn zone_inspector(&mut self, ui: &mut Ui, id: u32, actions: &mut Vec<Action>) {
        let Some(zone) = self.project.zone(id).cloned() else {
            return;
        };
        ui.heading("Zone");
        self.name_field(ui, "zone-name", zone.name.clone(), "Zone name", |p, t| {
            if let Some(z) = p.zone_mut(id) {
                z.name = t;
            }
        });
        let mut kind = ZoneKind::ALL
            .iter()
            .position(|&k| k == zone.kind)
            .unwrap_or(0);
        let names = ZoneKind::ALL.map(|k| k.name());
        if ui.dropdown("zone-kind", &mut kind, &names).changed {
            self.edit(|p| p.zone_mut(id).map(|z| z.kind = ZoneKind::ALL[kind]));
        }
        if ui
            .button_with(ButtonKind::Ghost, None, "Reset floor for this kind")
            .clicked
        {
            self.edit(|p| {
                p.zone_mut(id)
                    .map(|z| z.props = Zone::starter_floor(z.kind))
            });
            self.prop = None;
        }

        ui.heading("Boxes");
        let mut pick = None;
        for (i, prop) in zone.props.iter().enumerate() {
            let (tag, color) = if prop.solid {
                ("solid", ui.theme.text_dim)
            } else {
                ("ghost", ui.theme.text_faint)
            };
            if ui
                .list_row(
                    ("prop", i),
                    0,
                    None,
                    Some(Icon::Box),
                    &format!("Box {}", i + 1),
                    self.prop == Some(i),
                    Some((tag, color)),
                )
                .clicked
            {
                pick = Some(i);
            }
        }
        if let Some(i) = pick {
            self.prop = Some(i);
            self.view.stale = true;
        }
        if ui
            .button_with(ButtonKind::Normal, Some(Icon::Plus), "Add box")
            .on_hover_text(ui, "Or press A over the floor")
            .clicked
        {
            self.prop = self.edit(|p| {
                let z = p.zone_mut(id)?;
                z.props
                    .push(Prop::solid([0.0, 0.5, -5.0], [1.0; 3], PALETTE[3]));
                Some(z.props.len() - 1)
            });
        }

        if let Some(i) = self.prop.filter(|&i| i < zone.props.len()) {
            self.prop_inspector(ui, id, i, &zone.props[i]);
        }

        ui.heading("Groups");
        let holders: Vec<String> = self
            .project
            .groups
            .iter()
            .filter(|g| g.children.contains(&Item::Zone(id)))
            .map(|g| g.name.clone())
            .collect();
        if holders.is_empty() {
            ui.caption("In the free-mode pool (not in any group).");
        } else {
            ui.caption(&format!("In: {}", holders.join(", ")));
        }
        let groups: Vec<(u32, String)> = self
            .project
            .groups
            .iter()
            .filter(|g| self.project.can_add(g.id, Item::Zone(id)))
            .map(|g| (g.id, g.name.clone()))
            .collect();
        if !groups.is_empty() {
            let names: Vec<&str> = groups.iter().map(|(_, n)| n.as_str()).collect();
            self.pick_group = self.pick_group.min(names.len() - 1);
            ui.dropdown("add-to-group", &mut self.pick_group, &names);
            if ui
                .button_with(ButtonKind::Normal, Some(Icon::Folder), "Add to group")
                .clicked
            {
                let gid = groups[self.pick_group].0;
                self.edit(|p| p.group_mut(gid).map(|g| g.children.push(Item::Zone(id))));
            }
        }

        self.selection_actions(ui, actions, "This zone");
    }

    fn prop_inspector(&mut self, ui: &mut Ui, zone: u32, i: usize, prop: &Prop) {
        let axes = ui.theme.axis;
        let mut edited = prop.clone();
        let mut started = false;
        let mut changed = false;
        for (title, values, min) in [
            ("Position", &mut edited.position, f32::MIN),
            ("Size", &mut edited.size, 0.05),
        ] {
            ui.heading(title);
            ui.columns(3, |ui| {
                for (a, name) in ["X", "Y", "Z"].iter().enumerate() {
                    let r = ui.drag_value(
                        &format!("{title}{name}"),
                        Some((name, axes[a])),
                        &mut values[a],
                        0.25,
                        2,
                    );
                    started |= r.pressed;
                    changed |= r.changed;
                    values[a] = values[a].max(min);
                }
            });
        }
        ui.heading("Physics");
        let r = ui.toggle(&mut edited.solid, "Solid (has mass)");
        started |= r.pressed;
        changed |= r.changed;
        ui.caption(if edited.solid {
            "The runner stands on it and crashes into it."
        } else {
            "Decoration: the runner passes through it."
        });
        ui.heading("Color");
        let r = ui.color_swatches(&mut edited.color, &PALETTE);
        started |= r.pressed;
        changed |= r.changed;
        if started {
            self.history.record(&self.project);
        }
        if changed {
            if let Some(p) = self.project.zone_mut(zone).and_then(|z| z.props.get_mut(i)) {
                *p = edited;
            }
            self.dirty = true;
            self.view.stale = true;
        }
        ui.columns(2, |ui| {
            if ui
                .button_with(ButtonKind::Normal, Some(Icon::Copy), "Duplicate")
                .on_hover_text(ui, "Ctrl+D")
                .clicked
            {
                self.prop = self.edit(|p| {
                    let z = p.zone_mut(zone)?;
                    let mut copy = z.props.get(i)?.clone();
                    copy.position[0] += 1.0;
                    z.props.push(copy);
                    Some(z.props.len() - 1)
                });
            }
            if ui
                .button_with(ButtonKind::Danger, Some(Icon::Trash), "Remove")
                .on_hover_text(ui, "Delete")
                .clicked
            {
                self.edit(|p| p.zone_mut(zone).map(|z| z.props.remove(i)));
                self.prop = None;
            }
        });
    }

    fn group_inspector(&mut self, ui: &mut Ui, id: u32, actions: &mut Vec<Action>) {
        let Some(group) = self.project.group(id).cloned() else {
            return;
        };
        ui.heading("Group");
        self.name_field(
            ui,
            "group-name",
            group.name.clone(),
            "Group name",
            |p, t| {
                if let Some(g) = p.group_mut(id) {
                    g.name = t;
                }
            },
        );
        ui.heading("Order");
        let mut order = usize::from(group.order == Order::Shuffle);
        if ui
            .segmented(
                "order",
                &mut order,
                &[
                    (Some(Icon::Sequence), "Sequence"),
                    (Some(Icon::Shuffle), "Shuffle"),
                ],
            )
            .changed
        {
            self.edit(|p| {
                p.group_mut(id).map(|g| {
                    g.order = if order == 1 {
                        Order::Shuffle
                    } else {
                        Order::Sequence
                    }
                })
            });
        }
        ui.caption(match group.order {
            Order::Sequence => "Children run in the order below.",
            Order::Shuffle => "Children run in a new random order each time.",
        });

        ui.heading("Contents");
        if group.children.is_empty() {
            ui.caption("Empty. Add zones or groups below.");
        }
        let last = group.children.len().saturating_sub(1);
        for (i, child) in group.children.iter().enumerate() {
            let label = match *child {
                Item::Zone(z) => self.project.zone(z).map(|z| z.name.clone()),
                Item::Group(g) => self.project.group(g).map(|g| format!("{} (group)", g.name)),
            }
            .unwrap_or_else(|| "missing".into());
            let mut op = None;
            ui.push_id(("content", i), |ui| {
                ui.horizontal(|ui| {
                    ui.label_colored(&format!("{}.", i + 1), ui.theme.text_faint);
                    if ui.icon_button(Icon::ArrowUp, "Move up", false).clicked && i > 0 {
                        op = Some((i, i - 1));
                    }
                    if ui.icon_button(Icon::ArrowDown, "Move down", false).clicked && i < last {
                        op = Some((i, i + 1));
                    }
                    if ui
                        .icon_button(Icon::Close, "Remove from group", false)
                        .clicked
                    {
                        op = Some((i, usize::MAX));
                    }
                    ui.label(&label);
                });
            });
            if let Some((from, to)) = op {
                self.edit(|p| {
                    let g = p.group_mut(id)?;
                    if to == usize::MAX {
                        g.children.remove(from);
                    } else {
                        g.children.swap(from, to);
                    }
                    Some(())
                });
            }
        }
        let candidates: Vec<(Item, String)> = self
            .project
            .zones
            .iter()
            .map(|z| (Item::Zone(z.id), z.name.clone()))
            .chain(
                self.project
                    .groups
                    .iter()
                    .map(|g| (Item::Group(g.id), format!("{} (group)", g.name))),
            )
            .filter(|(item, _)| self.project.can_add(id, *item))
            .collect();
        if !candidates.is_empty() {
            let names: Vec<&str> = candidates.iter().map(|(_, n)| n.as_str()).collect();
            self.pick_child = self.pick_child.min(names.len() - 1);
            ui.dropdown("add-child", &mut self.pick_child, &names);
            if ui
                .button_with(ButtonKind::Normal, Some(Icon::Plus), "Add to this group")
                .clicked
            {
                let item = candidates[self.pick_child].0;
                self.edit(|p| p.group_mut(id).map(|g| g.children.push(item)));
            }
        }
        self.selection_actions(ui, actions, "This group");
    }

    fn selection_actions(&mut self, ui: &mut Ui, actions: &mut Vec<Action>, what: &str) {
        ui.heading(what);
        ui.columns(2, |ui| {
            if ui
                .button_with(ButtonKind::Normal, Some(Icon::Copy), "Duplicate")
                .clicked
            {
                self.prop = None;
                actions.push(Action::Duplicate);
            }
            if ui
                .button_with(ButtonKind::Danger, Some(Icon::Trash), "Delete")
                .clicked
            {
                self.prop = None;
                actions.push(Action::Delete);
            }
        });
    }

    fn project_inspector(&mut self, ui: &mut Ui) {
        ui.heading("Project");
        self.name_field(
            ui,
            "project-name",
            self.project.name.clone(),
            "Project name",
            |p, t| p.name = t,
        );
        ui.caption(&format!(
            "{} zones, {} groups, {} in the free-mode pool.",
            self.project.zones.len(),
            self.project.groups.len(),
            self.project.roots().len()
        ));
        ui.heading("Getting around");
        ui.caption(
            "Select a zone or group in the Scene panel.\n\
             Hold the right mouse button: look, WASD/Q/E fly.\n\
             Wheel: move forward and back.  F: focus.\n\
             Click a box, drag its arrows to move it.\n\
             A: add a box under the cursor.\n\
             P: run preview.  Ctrl+Z / Ctrl+Y: undo / redo.",
        );
    }

    // ------------------------------------------------------- viewport overlay

    fn card(ui: &mut Ui, rect: Rect) {
        let t = ui.theme.clone();
        ui.paint(DrawCommand::Shadow {
            rect,
            radius: t.radius,
            blur: 12.0,
            color: t.shadow.with_alpha(t.shadow.a * 0.6),
        });
        ui.paint(DrawCommand::RoundedRect {
            rect,
            radius: t.radius,
            fill: t.panel.with_alpha(0.92),
            border: 1.0,
            border_color: t.border,
        });
    }

    fn viewport_overlay(&mut self, ui: &mut Ui, rect: Rect) {
        let t = ui.theme.clone();
        let size = t.text_size;
        if let Some(preview) = &self.preview {
            let text = format!("{} m", preview.distance() as i32);
            let w = font::text_width(&text, 30.0) + 40.0;
            let card = Rect::new(rect.x + (rect.w - w) / 2.0, rect.y + 16.0, w, 46.0);
            Self::card(ui, card);
            ui.text_at(card.x + 20.0, card.y + 10.0, 30.0, t.text, &text);
            let hint = "Arrows / A D steer and turn   Space jump   R restart   Esc stop";
            let hw = font::text_width(hint, t.small_text_size);
            ui.text_at(
                rect.x + (rect.w - hw) / 2.0,
                rect.bottom() - 26.0,
                t.small_text_size,
                Color::rgba(1.0, 1.0, 1.0, 0.85),
                hint,
            );
            if let Some(death) = preview.death() {
                let title = match death {
                    Death::Crash => "Crashed",
                    Death::Fall => "Fell off the track",
                };
                let sub = "R to run again, Esc to stop";
                let w = font::text_width(title, 30.0).max(font::text_width(sub, size)) + 60.0;
                let card = Rect::new(
                    rect.x + (rect.w - w) / 2.0,
                    rect.y + rect.h / 2.0 - 60.0,
                    w,
                    110.0,
                );
                Self::card(ui, card);
                ui.text_at(
                    card.x + (w - font::text_width(title, 30.0)) / 2.0,
                    card.y + 22.0,
                    30.0,
                    t.danger,
                    title,
                );
                ui.text_at(
                    card.x + (w - font::text_width(sub, size)) / 2.0,
                    card.y + 66.0,
                    size,
                    t.text_dim,
                    sub,
                );
            }
            return;
        }
        let (kind, name, detail) = match self.selection {
            Selection::Zone(id) => match self.project.zone(id) {
                Some(z) => (
                    "ZONE",
                    z.name.clone(),
                    format!("{} · {} boxes", z.kind.name(), z.props.len()),
                ),
                None => return,
            },
            Selection::Group(id) => match self.project.group(id) {
                Some(g) => (
                    "GROUP",
                    g.name.clone(),
                    format!(
                        "{} items · {}",
                        g.children.len(),
                        if g.order == Order::Shuffle {
                            "shuffled at runtime"
                        } else {
                            "in sequence"
                        }
                    ),
                ),
                None => return,
            },
            Selection::Nothing => (
                "PROJECT",
                self.project.name.clone(),
                "Select a zone or group".into(),
            ),
        };
        let w =
            font::text_width(&name, size).max(font::text_width(&detail, t.small_text_size)) + 90.0;
        let card = Rect::new(rect.x + 14.0, rect.y + 14.0, w, 56.0);
        Self::card(ui, card);
        ui.text_at(
            card.x + 14.0,
            card.y + 12.0,
            t.small_text_size,
            t.accent_hover,
            kind,
        );
        ui.text_at(card.x + 70.0, card.y + 8.0, size, t.text, &name);
        ui.text_at(
            card.x + 70.0,
            card.y + 34.0,
            t.small_text_size,
            t.text_dim,
            &detail,
        );

        let hint = "RMB look · WASD fly · F focus · A add box";
        let hw = font::text_width(hint, t.small_text_size) + 24.0;
        let hint_card = Rect::new(rect.right() - hw - 14.0, rect.y + 14.0, hw, 26.0);
        Self::card(ui, hint_card);
        ui.text_at(
            hint_card.x + 12.0,
            hint_card.y + 9.0,
            t.small_text_size,
            t.text_dim,
            hint,
        );
    }

    // ------------------------------------------------------------ status bar

    fn status_bar(&mut self, ui: &mut Ui, rect: Rect) {
        let counts = format!(
            "{} zones · {} groups{}",
            self.project.zones.len(),
            self.project.groups.len(),
            if self.dirty {
                " · unsaved changes"
            } else {
                ""
            }
        );
        let status = self.status.clone();
        ui.bar("status", rect, |ui| {
            ui.caption(&status);
        });
        let w = font::text_width(&counts, ui.theme.small_text_size);
        let color = if self.dirty {
            ui.theme.warning
        } else {
            ui.theme.text_dim
        };
        ui.text_at(
            rect.right() - w - 14.0,
            rect.y + 10.0,
            ui.theme.small_text_size,
            color,
            &counts,
        );
    }

    // --------------------------------------------------------------- dialogs

    fn dialogs(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let Some(what) = self.confirm else { return };
        let (title, body) = match what {
            Confirm::DeleteZone(id) => (
                "Delete zone?",
                format!(
                    "\"{}\" will be removed from the project\nand from every group. You can undo this.",
                    self.project.zone(id).map_or("?", |z| z.name.as_str())
                ),
            ),
            Confirm::DeleteGroup(id) => (
                "Delete group?",
                format!(
                    "\"{}\" will be removed. Its zones stay in\nthe project. You can undo this.",
                    self.project.group(id).map_or("?", |g| g.name.as_str())
                ),
            ),
        };
        let mut close = ui.input().keys.contains(&UiKey::Escape);
        let mut confirm = ui.input().keys.contains(&UiKey::Enter);
        ui.modal(title, 420.0, |ui| {
            ui.caption(&body);
            ui.space(12.0);
            ui.columns(2, |ui| {
                close |= ui.button("Cancel").clicked;
                confirm |= ui
                    .button_with(ButtonKind::Danger, Some(Icon::Trash), "Delete")
                    .clicked;
            });
        });
        if confirm {
            actions.push(Action::Confirmed(what));
        } else if close {
            self.confirm = None;
        }
    }
}
