use crate::{camera::Camera, color::Color, framebuffer::Framebuffer, model::Model, triangle};
use nalgebra_glm::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RenderMode {
    Solid,
    Wireframe,
    Hologram,
}

impl RenderMode {
    pub fn next(self) -> Self {
        match self {
            Self::Solid => Self::Wireframe,
            Self::Wireframe => Self::Hologram,
            Self::Hologram => Self::Solid,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Solid => "Sólido",
            Self::Wireframe => "Malla",
            Self::Hologram => "Holograma",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Viewport {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Shared CPU pipeline for the three previews and the inspection view.
pub fn draw_model(
    framebuffer: &mut Framebuffer,
    model: &Model,
    camera: &Camera,
    mode: RenderMode,
    viewport: Viewport,
    fallback_color: Option<Color>,
) {
    framebuffer.set_clip(viewport.x, viewport.y, viewport.width, viewport.height);
    let (right, up, forward) = camera.basis();
    let scale =
        0.88 * viewport.width.min(viewport.height) as f32 / (2.0 * model.radius) * camera.zoom;
    // Lighting normals are computed before screen scaling, preserving their shape.
    let view: Vec<Vec3> = model
        .vertices
        .iter()
        .map(|vertex| {
            let relative = vertex - model.center;
            Vec3::new(
                relative.dot(&right),
                relative.dot(&up),
                -relative.dot(&forward),
            )
        })
        .collect();
    let screen: Vec<Vec3> = view
        .iter()
        .map(|v| {
            Vec3::new(
                viewport.x as f32 + viewport.width as f32 * 0.5 + v.x * scale,
                viewport.y as f32 + viewport.height as f32 * 0.5 - v.y * scale,
                v.z,
            )
        })
        .collect();
    let light = Vec3::new(-0.4, 0.8, 1.0).normalize();
    let half = (light + Vec3::new(0.0, 0.0, 1.0)).normalize();
    for face in &model.faces {
        let [i, j, k] = face.indices;
        let [a, b, c] = [screen[i], screen[j], screen[k]];
        if mode == RenderMode::Solid {
            let normal = (view[j] - view[i]).cross(&(view[k] - view[i]));
            if normal.norm_squared() < f32::EPSILON {
                continue;
            }
            let mut normal = normal.normalize();
            if normal.z < 0.0 {
                normal = -normal;
            }
            let diffuse = normal.dot(&light).max(0.0);
            let highlight = normal.dot(&half).max(0.0).powf(28.0) * 0.30;
            let base = fallback_color.unwrap_or(face.color);
            let intensity = 0.40 + 0.85 * diffuse;
            let surface = base.shaded(intensity);
            let rim = (1.0 - normal.z).powf(3.0) * 0.10;
            let lift = |channel: u8| (channel as f32 + 255.0 * (highlight + rim)).min(255.0) as u8;
            triangle::fill(
                framebuffer,
                &a,
                &b,
                &c,
                Color::new(lift(surface.r), lift(surface.g), lift(surface.b)),
            );
        } else {
            let color = if mode == RenderMode::Hologram {
                0x60e6ef
            } else {
                0x93bed2
            };
            let depth = mode == RenderMode::Wireframe;
            triangle::line(framebuffer, &a, &b, color, depth);
            triangle::line(framebuffer, &b, &c, color, depth);
            triangle::line(framebuffer, &c, &a, color, depth);
        }
    }
    framebuffer.reset_clip();
}
