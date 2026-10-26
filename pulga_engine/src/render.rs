//! Turns a frame of engine state into pixels: the [`World`] through the
//! camera, overlay entities on top, then the UI.
//!
//! The drawing itself belongs to the libraries: meshes go through
//! `pulga_win_lib`'s [`MeshRenderer`], UI [`DrawCommand`]s through
//! `pulga_gui_lib`'s [`GuiRenderer`]. The engine's part is owning the GPU
//! meshes behind [`MeshId`]s and walking the scene.

use std::marker::PhantomData;

use pulga_gui_lib::{DrawCommand, render::GuiRenderer};
use pulga_win_lib::{
    Camera, gl_mesh::GlMesh, obj::Mesh, primitives, render as gfx, render::MeshRenderer,
};

use crate::Color;
use crate::scene::World;

/// Handle to a mesh uploaded with [`Renderer::add_mesh`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshId(usize);

#[cfg(test)]
impl MeshId {
    /// A placeholder id for tests that never draw.
    pub(crate) fn test_id() -> Self {
        MeshId(0)
    }
}

/// Distance fog: far geometry fades into a color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    /// The color far things fade into (usually the clear color).
    pub color: Color,
    /// How fast fog thickens, per world unit (about `1.5 / density` units
    /// away a mesh is mostly fogged).
    pub density: f32,
}

/// Owns the engine's GPU resources.
///
/// Only the engine creates one, on the GL thread, after the GL context is
/// current. That is what lets [`Renderer::add_mesh`] be a safe function. The
/// marker keeps it `!Send` so it can't leave that thread.
pub struct Renderer {
    /// Background color the frame is cleared to.
    pub clear_color: Color,
    /// Distance fog over the world (not over overlay entities), if any.
    pub fog: Option<Fog>,
    meshes: Vec<GlMesh>,
    mesh_renderer: MeshRenderer,
    gui: GuiRenderer,
    _gl_thread: PhantomData<*const ()>,
}

const CUBE: MeshId = MeshId(0);
const SPHERE: MeshId = MeshId(1);

impl Renderer {
    /// Sets up the library pipelines and uploads the built-in meshes.
    ///
    /// # Safety
    /// A GL 4.1 context must be current on this thread.
    pub(crate) unsafe fn new() -> Self {
        unsafe {
            Self {
                clear_color: Color::rgb(0.53, 0.75, 0.92),
                fog: None,
                meshes: vec![
                    GlMesh::upload(&primitives::cube(1.0)),
                    GlMesh::upload(&primitives::sphere(0.5, 12, 18)),
                ],
                mesh_renderer: MeshRenderer::new(),
                gui: GuiRenderer::new(),
                _gl_thread: PhantomData,
            }
        }
    }

    /// Uploads `mesh` to the GPU; entities draw it by holding the returned
    /// id in [`crate::scene::Entity::mesh`].
    pub fn add_mesh(&mut self, mesh: &Mesh) -> MeshId {
        // SAFETY: a Renderer only exists on the GL thread with the context
        // current (see `Renderer::new`).
        self.meshes.push(unsafe { GlMesh::upload(mesh) });
        MeshId(self.meshes.len() - 1)
    }

    /// The built-in unit cube centered on the origin. Scaled by an entity's
    /// transform, it draws every box-shaped thing (zones are made of these).
    pub fn cube(&self) -> MeshId {
        CUBE
    }

    /// The built-in sphere of diameter 1 centered on the origin.
    pub fn sphere(&self) -> MeshId {
        SPHERE
    }

    /// Clears the frame, draws every visible entity that has a mesh
    /// (overlay ones last, on top), then `ui`. `screen` is the window size
    /// in pixels.
    pub(crate) fn draw(
        &mut self,
        world: &World,
        camera: &Camera,
        ui: &[DrawCommand],
        screen: (f32, f32),
    ) {
        let rgba = |c: Color| [c.r, c.g, c.b, c.a];
        // SAFETY: see `add_mesh`.
        unsafe {
            gfx::clear(rgba(self.clear_color));
            for overlay in [false, true] {
                if overlay {
                    gfx::clear_depth();
                }
                self.mesh_renderer.begin(&camera.proj(), &camera.view());
                match self.fog.filter(|_| !overlay) {
                    Some(fog) => self
                        .mesh_renderer
                        .set_fog([fog.color.r, fog.color.g, fog.color.b], fog.density),
                    None => self.mesh_renderer.set_fog([0.0; 3], 0.0),
                }
                for (id, entity) in world.iter() {
                    if entity.overlay != overlay || !entity.visible {
                        continue;
                    }
                    let Some(MeshId(mesh)) = entity.mesh else {
                        continue;
                    };
                    let model = world.world_matrix(id).expect("id comes from world.iter()");
                    self.mesh_renderer
                        .draw(&self.meshes[mesh], &model, rgba(entity.color));
                }
            }
            self.gui.draw(ui, screen.0, screen.1);
        }
    }
}
