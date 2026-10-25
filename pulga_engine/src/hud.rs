//! Screen-space text overlay (distance meter, game over, ...).
//!
//! Immediate mode: the engine clears the [`Hud`] at the start of every frame,
//! the game queues whatever text it wants shown this frame, and the renderer
//! draws it on top of the 3D scene. Coordinates are pixels from the window's
//! top-left corner, y pointing down.
//!
//! ```
//! use pulga_engine::{Color, hud::Hud};
//!
//! let mut hud = Hud::default();
//! let white = Color::rgb(1.0, 1.0, 1.0);
//! hud.text(16.0, 16.0, 32.0, white, "DISTANCE 42");
//!
//! // Center a line horizontally on a 1280-pixel-wide window.
//! let w = Hud::text_width("GAME OVER", 64.0);
//! hud.text((1280.0 - w) / 2.0, 300.0, 64.0, white, "GAME OVER");
//! ```

use pulga_gui_lib::Color;

use crate::font::{self, CELL_H, CELL_W, GLYPHS};

/// Floats per HUD vertex: position (2), atlas UV (2), color (4).
pub(crate) const VERTEX_FLOATS: usize = 8;

struct TextItem {
    x: f32,
    y: f32,
    size: f32,
    color: Color,
    text: String,
}

/// This frame's queued text.
#[derive(Default)]
pub struct Hud {
    items: Vec<TextItem>,
}

impl Hud {
    /// Queues `text` with its top-left corner at (`x`, `y`) pixels. `size` is
    /// the line height in pixels; the font's pixels scale by `size / 8`, so
    /// multiples of 8 stay crisp. `\n` starts a new line.
    pub fn text(&mut self, x: f32, y: f32, size: f32, color: Color, text: &str) {
        self.items.push(TextItem {
            x,
            y,
            size,
            color,
            text: text.to_owned(),
        });
    }

    /// Width in pixels of the longest line of `text` at `size`, for
    /// centering or right-aligning.
    pub fn text_width(text: &str, size: f32) -> f32 {
        let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
        longest as f32 * CELL_W as f32 * size / CELL_H as f32
    }

    /// Drops everything queued (the engine does this every frame).
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Two triangles per visible glyph, as interleaved
    /// [`VERTEX_FLOATS`]-float vertices ready for the HUD shader.
    pub(crate) fn vertices(&self) -> Vec<f32> {
        let atlas_w = (GLYPHS.len() * CELL_W) as f32;
        let mut out = Vec::new();
        for item in &self.items {
            let px = item.size / CELL_H as f32; // screen pixels per font pixel
            let (cw, ch) = (CELL_W as f32 * px, CELL_H as f32 * px);
            let Color { r, g, b, a } = item.color;
            for (line_no, line) in item.text.lines().enumerate() {
                let y0 = item.y + line_no as f32 * ch;
                for (col, c) in line.chars().enumerate() {
                    if c == ' ' {
                        continue;
                    }
                    let x0 = item.x + col as f32 * cw;
                    let u0 = (font::glyph_index(c) * CELL_W) as f32 / atlas_w;
                    let u1 = u0 + CELL_W as f32 / atlas_w;
                    let corner = |x: f32, y: f32, u: f32, v: f32| [x, y, u, v, r, g, b, a];
                    let (tl, tr) = (corner(x0, y0, u0, 0.0), corner(x0 + cw, y0, u1, 0.0));
                    let (bl, br) = (
                        corner(x0, y0 + ch, u0, 1.0),
                        corner(x0 + cw, y0 + ch, u1, 1.0),
                    );
                    for v in [tl, bl, br, tl, br, tr] {
                        out.extend_from_slice(&v);
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);

    #[test]
    fn six_vertices_per_glyph_and_none_for_spaces() {
        let mut hud = Hud::default();
        hud.text(0.0, 0.0, 8.0, WHITE, "A B");
        assert_eq!(hud.vertices().len(), 2 * 6 * VERTEX_FLOATS);
    }

    #[test]
    fn glyphs_advance_by_one_cell_and_lines_by_one_line() {
        let mut hud = Hud::default();
        hud.text(10.0, 20.0, 16.0, WHITE, "AB\nC");
        let v = hud.vertices();
        let x_of = |glyph: usize| v[glyph * 6 * VERTEX_FLOATS];
        let y_of = |glyph: usize| v[glyph * 6 * VERTEX_FLOATS + 1];
        // size 16 = 2 screen pixels per font pixel: cells are 12 wide, 16 tall.
        assert_eq!((x_of(0), y_of(0)), (10.0, 20.0));
        assert_eq!((x_of(1), y_of(1)), (22.0, 20.0));
        assert_eq!((x_of(2), y_of(2)), (10.0, 36.0));
    }

    #[test]
    fn text_width_uses_the_longest_line() {
        assert_eq!(Hud::text_width("ABC\nA", 8.0), 18.0);
        assert_eq!(Hud::text_width("", 8.0), 0.0);
    }

    #[test]
    fn clear_empties_the_queue() {
        let mut hud = Hud::default();
        hud.text(0.0, 0.0, 8.0, WHITE, "X");
        hud.clear();
        assert!(hud.vertices().is_empty());
    }
}
