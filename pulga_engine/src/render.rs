//! Turns a frame of engine state into pixels: the [`World`] through the
//! camera, then the UI on top.
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

/// Owns the engine's GPU resources.
///
/// Only the engine creates one, on the GL thread, after the GL context is
/// current. That is what lets [`Renderer::add_mesh`] be a safe function. The
/// marker keeps it `!Send` so it can't leave that thread.
pub struct Renderer {
    /// Background color the frame is cleared to.
    pub clear_color: Color,
    meshes: Vec<GlMesh>,
    cube: MeshId,
    mesh_renderer: MeshRenderer,
    gui: GuiRenderer,
    _gl_thread: PhantomData<*const ()>,
}

impl Renderer {
    /// Sets up the library pipelines and uploads the built-in unit cube.
    ///
    /// # Safety
    /// A GL 4.1 context must be current on this thread.
    pub(crate) unsafe fn new() -> Self {
        unsafe {
            Self {
                clear_color: Color::rgb(0.53, 0.75, 0.92),
                meshes: vec![GlMesh::upload(&primitives::cube(1.0))],
                cube: MeshId(0),
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
    /// transform, it draws every box-shaped thing (tiles are made of these).
    pub fn cube(&self) -> MeshId {
        self.cube
    }

    /// Clears the frame, draws every visible entity that has a mesh, then
    /// `ui` on top. `screen` is the window size in pixels.
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
            self.mesh_renderer.begin(&camera.proj(), &camera.view());
            for (id, entity) in world.iter() {
                let Some(MeshId(mesh)) = entity.mesh.filter(|_| entity.visible) else {
                    continue;
                };
                let model = world.world_matrix(id).expect("id comes from world.iter()");
                self.mesh_renderer
                    .draw(&self.meshes[mesh], &model, rgba(entity.color));
            }
            self.gui.draw(ui, screen.0, screen.1);
        }
    }
}
