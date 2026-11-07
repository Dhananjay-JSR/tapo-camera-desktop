//! GPU video: upload I420 planes as textures and convert YUV→RGB (and scale) in a fragment
//! shader, via an egui glow paint callback. Keeps the CPU out of per-pixel work.

use crate::decode::YuvFrame;
use eframe::egui_glow;
use eframe::glow::{self, HasContext};
use std::sync::{Arc, Mutex};

/// Shared video surface: the UI pushes the latest frame; the paint callback uploads and draws it.
#[derive(Clone)]
pub struct GpuVideo {
    frame: Arc<Mutex<Option<YuvFrame>>>,
    gl: Arc<Mutex<Option<GlObjects>>>,
}

struct GlObjects {
    program: glow::Program,
    vao: glow::VertexArray,
    tex: [glow::Texture; 3],
}

impl GpuVideo {
    pub fn new() -> Self {
        GpuVideo {
            frame: Arc::new(Mutex::new(None)),
            gl: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_frame(&self, f: YuvFrame) {
        *self.frame.lock().unwrap() = Some(f);
    }

    pub fn has_frame(&self) -> bool {
        self.frame.lock().unwrap().is_some()
    }

    /// Paint the current frame into `rect` (letterboxed by the caller's sizing).
    pub fn paint(&self, ui: &egui::Ui, rect: egui::Rect) {
        let frame = self.frame.clone();
        let gl_state = self.gl.clone();
        let cb = egui_glow::CallbackFn::new(move |info, painter| {
            let gl = painter.gl();
            let mut guard = gl_state.lock().unwrap();
            if guard.is_none() {
                match GlObjects::new(gl) {
                    Some(o) => *guard = Some(o),
                    None => return,
                }
            }
            let objs = guard.as_ref().unwrap();
            if let Some(f) = frame.lock().unwrap().as_ref() {
                unsafe { objs.draw(gl, f, &info) };
            }
        });
        ui.painter().add(egui::PaintCallback {
            rect,
            callback: Arc::new(cb),
        });
    }
}

const VERT: &str = r#"#version 330 core
out vec2 uv;
void main() {
    // Fullscreen triangle from the vertex id; flip V so texture row 0 is at the top.
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    uv = vec2(p.x, 1.0 - p.y);
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

const FRAG: &str = r#"#version 330 core
in vec2 uv;
out vec4 color;
uniform sampler2D texY;
uniform sampler2D texU;
uniform sampler2D texV;
void main() {
    float y = texture(texY, uv).r;
    float u = texture(texU, uv).r - 0.5;
    float v = texture(texV, uv).r - 0.5;
    // BT.601 full-range
    float r = y + 1.402 * v;
    float g = y - 0.344 * u - 0.714 * v;
    float b = y + 1.772 * u;
    color = vec4(r, g, b, 1.0);
}
"#;

impl GlObjects {
    fn new(gl: &glow::Context) -> Option<Self> {
        unsafe {
            let program = gl.create_program().ok()?;
            for (ty, src) in [(glow::VERTEX_SHADER, VERT), (glow::FRAGMENT_SHADER, FRAG)] {
                let sh = gl.create_shader(ty).ok()?;
                gl.shader_source(sh, src);
                gl.compile_shader(sh);
                if !gl.get_shader_compile_status(sh) {
                    eprintln!("shader compile: {}", gl.get_shader_info_log(sh));
                    return None;
                }
                gl.attach_shader(program, sh);
                gl.delete_shader(sh);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                eprintln!("program link: {}", gl.get_program_info_log(program));
                return None;
            }
            let vao = gl.create_vertex_array().ok()?;
            let tex = [make_tex(gl)?, make_tex(gl)?, make_tex(gl)?];
            Some(GlObjects { program, vao, tex })
        }
    }

    unsafe fn draw(&self, gl: &glow::Context, f: &YuvFrame, info: &egui::PaintCallbackInfo) {
        let vp = info.viewport_in_pixels();
        gl.viewport(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1); // R8 rows aren't 4-byte aligned
        gl.use_program(Some(self.program));
        let planes = [
            (self.tex[0], &f.y, f.w as i32, f.h as i32, "texY", 0),
            (
                self.tex[1],
                &f.u,
                (f.w / 2) as i32,
                (f.h / 2) as i32,
                "texU",
                1,
            ),
            (
                self.tex[2],
                &f.v,
                (f.w / 2) as i32,
                (f.h / 2) as i32,
                "texV",
                2,
            ),
        ];
        for (tex, data, w, h, name, unit) in planes {
            gl.active_texture(glow::TEXTURE0 + unit as u32);
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R8 as i32,
                w,
                h,
                0,
                glow::RED,
                glow::UNSIGNED_BYTE,
                Some(data.as_slice()),
            );
            if let Some(loc) = gl.get_uniform_location(self.program, name) {
                gl.uniform_1_i32(Some(&loc), unit);
            }
        }
        gl.bind_vertex_array(Some(self.vao));
        gl.draw_arrays(glow::TRIANGLES, 0, 3);
    }
}

unsafe fn make_tex(gl: &glow::Context) -> Option<glow::Texture> {
    let t = gl.create_texture().ok()?;
    gl.bind_texture(glow::TEXTURE_2D, Some(t));
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_MIN_FILTER,
        glow::LINEAR as i32,
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_MAG_FILTER,
        glow::LINEAR as i32,
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_WRAP_S,
        glow::CLAMP_TO_EDGE as i32,
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_WRAP_T,
        glow::CLAMP_TO_EDGE as i32,
    );
    Some(t)
}
