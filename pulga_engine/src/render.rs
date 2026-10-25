//! Draws the [`World`] and the [`Hud`] with OpenGL 4.1, using our own
//! shaders (`assets/shaders/`) and raw `gl` calls, per the subject.
//!
//! This is the only module of the engine that talks to GL; games never do.
//! It is OpenGL-only for now. A Vulkan backend would turn [`Renderer`] into
//! a trait with one implementation per API, which is only worth it once a
//! second backend exists.

use std::marker::PhantomData;

use pulga_gui_lib::Color;
use pulga_win_lib::{Camera, gl, gl_mesh::GlMesh, obj::Mesh, shader, transform};

use crate::font;
use crate::hud::{Hud, VERTEX_FLOATS};
use crate::scene::World;

/// Handle to a mesh uploaded with [`Renderer::add_mesh`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshId(usize);

/// Owns every GPU resource: shader programs, uploaded meshes, the font
/// atlas.
///
/// Only the engine creates one, on the GL thread, after the GL context is
/// current. That is what lets [`Renderer::add_mesh`] be a safe function. The
/// marker keeps it `!Send` so it can't leave that thread.
pub struct Renderer {
    /// Background color the frame is cleared to.
    pub clear_color: Color,
    meshes: Vec<GlMesh>,
    mesh_program: gl::types::GLuint,
    proj_loc: gl::types::GLint,
    view_loc: gl::types::GLint,
    model_loc: gl::types::GLint,
    color_loc: gl::types::GLint,
    hud_program: gl::types::GLuint,
    screen_loc: gl::types::GLint,
    hud_vao: gl::types::GLuint,
    hud_vbo: gl::types::GLuint,
    font_texture: gl::types::GLuint,
    _gl_thread: PhantomData<*const ()>,
}

impl Renderer {
    /// Compiles the shaders and uploads the font atlas.
    ///
    /// # Safety
    /// A GL 4.1 context must be current on this thread.
    pub(crate) unsafe fn new() -> Self {
        unsafe {
            let mesh_program = shader::link_program(
                include_str!("../assets/shaders/mesh.vert"),
                include_str!("../assets/shaders/mesh.frag"),
            )
            .expect("engine mesh shader failed to compile/link");
            let hud_program = shader::link_program(
                include_str!("../assets/shaders/hud.vert"),
                include_str!("../assets/shaders/hud.frag"),
            )
            .expect("engine HUD shader failed to compile/link");

            let (mut hud_vao, mut hud_vbo) = (0, 0);
            gl::GenVertexArrays(1, &mut hud_vao);
            gl::GenBuffers(1, &mut hud_vbo);
            gl::BindVertexArray(hud_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, hud_vbo);
            let stride = (VERTEX_FLOATS * size_of::<f32>()) as i32;
            for (location, size, offset) in [(0, 2, 0), (1, 2, 2), (2, 4, 4)] {
                gl::VertexAttribPointer(
                    location,
                    size,
                    gl::FLOAT,
                    gl::FALSE,
                    stride,
                    (offset * size_of::<f32>()) as *const _,
                );
                gl::EnableVertexAttribArray(location);
            }
            gl::BindVertexArray(0);

            let (pixels, w, h) = font::atlas();
            let mut font_texture = 0;
            gl::GenTextures(1, &mut font_texture);
            gl::BindTexture(gl::TEXTURE_2D, font_texture);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1); // rows aren't 4-byte multiples
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::R8 as i32,
                w as i32,
                h as i32,
                0,
                gl::RED,
                gl::UNSIGNED_BYTE,
                pixels.as_ptr().cast(),
            );
            // Nearest: keep the font's pixels square and crisp when scaled.
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);

            gl::Enable(gl::DEPTH_TEST);

            Self {
                clear_color: Color::rgb(0.53, 0.75, 0.92),
                meshes: Vec::new(),
                mesh_program,
                proj_loc: shader::uniform_location(mesh_program, "proj"),
                view_loc: shader::uniform_location(mesh_program, "view"),
                model_loc: shader::uniform_location(mesh_program, "model"),
                color_loc: shader::uniform_location(mesh_program, "color"),
                hud_program,
                screen_loc: shader::uniform_location(hud_program, "screen"),
                hud_vao,
                hud_vbo,
                font_texture,
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

    /// Clears the frame, then draws every visible entity that has a mesh and
    /// the HUD on top. `screen` is the window size in pixels.
    pub(crate) fn draw(&self, world: &World, camera: &Camera, hud: &Hud, screen: (u32, u32)) {
        // SAFETY: see `add_mesh`.
        unsafe {
            let Color { r, g, b, a } = self.clear_color;
            gl::ClearColor(r, g, b, a);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

            gl::UseProgram(self.mesh_program);
            let set_mat = |loc, m: &transform::Mat4| {
                gl::UniformMatrix4fv(loc, 1, gl::TRUE, transform::to_gl(m).as_ptr())
            };
            set_mat(self.proj_loc, &camera.proj());
            set_mat(self.view_loc, &camera.view());
            for (id, entity) in world.iter() {
                let Some(MeshId(mesh)) = entity.mesh.filter(|_| entity.visible) else {
                    continue;
                };
                let model = world.world_matrix(id).expect("id comes from world.iter()");
                set_mat(self.model_loc, &model);
                let Color { r, g, b, a } = entity.color;
                gl::Uniform4f(self.color_loc, r, g, b, a);
                self.meshes[mesh].draw();
            }

            let vertices = hud.vertices();
            if vertices.is_empty() {
                return;
            }
            gl::Disable(gl::DEPTH_TEST);
            gl::UseProgram(self.hud_program);
            gl::Uniform2f(self.screen_loc, screen.0 as f32, screen.1 as f32);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.font_texture);
            gl::BindVertexArray(self.hud_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.hud_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (vertices.len() * size_of::<f32>()) as isize,
                vertices.as_ptr().cast(),
                gl::STREAM_DRAW,
            );
            gl::DrawArrays(gl::TRIANGLES, 0, (vertices.len() / VERTEX_FLOATS) as i32);
            gl::BindVertexArray(0);
            gl::Enable(gl::DEPTH_TEST);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // SAFETY: see `add_mesh`. GlMesh frees its own buffers on drop.
        unsafe {
            gl::DeleteTextures(1, &self.font_texture);
            gl::DeleteBuffers(1, &self.hud_vbo);
            gl::DeleteVertexArrays(1, &self.hud_vao);
            gl::DeleteProgram(self.hud_program);
            gl::DeleteProgram(self.mesh_program);
        }
    }
}
