use crate::{
    camera::Camera, color::Color, flight::Flight, framebuffer::Framebuffer, model::Model, triangle,
};
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

/// Shared CPU pipeline for the three previews, inspection, and flight view.
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
    rasterize(framebuffer, model, &view, &screen, mode, fallback_color);
    framebuffer.reset_clip();
}

pub const FLIGHT_FOCAL: f32 = 660.0;

/// Render the actual ship orientation in the same perspective as the star field.
pub fn draw_flight_model(
    framebuffer: &mut Framebuffer,
    model: &Model,
    flight: &Flight,
    fallback_color: Option<Color>,
) {
    // All three source OBJs have their nose along +Z and their roof along +Y.
    let model_forward = Vec3::new(0.0, 0.0, 1.0);
    let model_up = Vec3::new(0.0, 1.0, 0.0);
    let model_right = model_forward.cross(&model_up).normalize();
    let (ship_right, ship_up, ship_forward) = flight.ship_basis();
    let (right, up, forward) = flight.camera.basis();
    let eye_to_ship = flight.position - flight.camera_position();
    let view: Vec<Vec3> = model
        .vertices
        .iter()
        .map(|vertex| {
            let local = (vertex - model.center) * (1.7 / model.radius);
            let world = eye_to_ship
                + ship_right * local.dot(&model_right)
                + ship_up * local.dot(&model_up)
                + ship_forward * local.dot(&model_forward);
            Vec3::new(world.dot(&right), world.dot(&up), -world.dot(&forward))
        })
        .collect();
    let screen: Vec<Vec3> = view
        .iter()
        .map(|v| {
            let depth = -v.z;
            Vec3::new(
                framebuffer.width as f32 * 0.5 + v.x * FLIGHT_FOCAL / depth,
                framebuffer.height as f32 * 0.5 - v.y * FLIGHT_FOCAL / depth,
                1.0 / depth,
            )
        })
        .collect();
    rasterize(
        framebuffer,
        model,
        &view,
        &screen,
        RenderMode::Solid,
        fallback_color,
    );
}

fn rasterize(
    framebuffer: &mut Framebuffer,
    model: &Model,
    view: &[Vec3],
    screen: &[Vec3],
    mode: RenderMode,
    fallback_color: Option<Color>,
) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ships_stay_visible_below_the_reticle_during_turns() {
        let ships = crate::fleet::load().unwrap();
        let mut flight = Flight::new();
        let mut fb = Framebuffer::new(1280, 840);
        flight.move_with_mouse(Some((1.0, -1.0)));
        for frame in 0..600 {
            flight.advance(1.0 / 60.0, (1.0, 0.0), false, false);
            if frame % 60 != 0 {
                continue;
            }
            for ship in &ships {
                fb.clear();
                draw_flight_model(&mut fb, &ship.model, &flight, ship.finish);
                let mut visible = 0;
                for (index, &pixel) in fb.pixels.iter().enumerate() {
                    if pixel != crate::framebuffer::BACKGROUND {
                        let (x, y) = (index % fb.width, index / fb.width);
                        assert!(
                            (350..930).contains(&x) && (440..730).contains(&y),
                            "{} left the chase view at frame {frame}: ({x}, {y})",
                            ship.name
                        );
                        visible += 1;
                    }
                }
                assert!(visible > 500, "{} disappeared at frame {frame}", ship.name);
            }
        }
    }
}
