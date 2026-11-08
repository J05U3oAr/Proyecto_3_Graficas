use crate::raster_batch::{self, Primitive, Surface};
use crate::{
    camera::Camera, color::Color, flight::Flight, framebuffer::Framebuffer, model::Model, triangle,
};
use nalgebra_glm::{Vec2, Vec3};

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
    rasterize(
        framebuffer,
        model,
        &view,
        &screen,
        mode,
        fallback_color,
        false,
    );
    framebuffer.reset_clip();
}

pub const FLIGHT_FOCAL: f32 = 660.0;
const NEAR_PLANE: f32 = 0.25;

#[derive(Clone, Copy)]
pub struct WorldTransform {
    pub position: Vec3,
    pub radius: f32,
    pub yaw: f32,
}

pub struct FlightView {
    eye: Vec3,
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    width: f32,
    height: f32,
}

impl FlightView {
    pub fn new(flight: &Flight, width: usize, height: usize) -> Self {
        let (right, up, forward) = flight.camera.basis();
        Self {
            eye: flight.camera_position(),
            right,
            up,
            forward,
            width: width as f32,
            height: height as f32,
        }
    }

    pub fn view_point(&self, world: Vec3) -> Vec3 {
        self.view_direction(world - self.eye)
    }

    pub fn view_direction(&self, relative: Vec3) -> Vec3 {
        Vec3::new(
            relative.dot(&self.right),
            relative.dot(&self.up),
            -relative.dot(&self.forward),
        )
    }

    pub fn project_view(&self, point: Vec3) -> Option<Vec3> {
        let depth = -point.z;
        (depth >= NEAR_PLANE).then(|| {
            Vec3::new(
                self.width * 0.5 + point.x * FLIGHT_FOCAL / depth,
                self.height * 0.5 - point.y * FLIGHT_FOCAL / depth,
                1.0 / depth,
            )
        })
    }

    pub fn project(&self, world: Vec3) -> Option<Vec3> {
        self.project_view(self.view_point(world))
    }

    pub fn sphere_visible(&self, center: Vec3, radius: f32) -> bool {
        let point = self.view_point(center);
        let depth = -point.z;
        let horizontal = self.width * 0.5 / FLIGHT_FOCAL;
        let vertical = self.height * 0.5 / FLIGHT_FOCAL;
        depth + radius >= NEAR_PLANE
            && point.x.abs() <= depth * horizontal + radius * (1.0 + horizontal * horizontal).sqrt()
            && point.y.abs() <= depth * vertical + radius * (1.0 + vertical * vertical).sqrt()
    }
}

/// Opaque world objects and the ship share reciprocal depth in the framebuffer.
pub fn draw_world_model(
    fb: &mut Framebuffer,
    model: &Model,
    camera: &FlightView,
    transform: WorldTransform,
) {
    draw_world_model_lit(fb, model, camera, transform, None);
}

pub fn draw_world_model_lit(
    fb: &mut Framebuffer,
    model: &Model,
    camera: &FlightView,
    transform: WorldTransform,
    sun: Option<Vec3>,
) {
    draw_world_model_shaded(
        fb,
        model,
        camera,
        transform,
        sun,
        crate::shaders::SurfaceEffect::None,
    );
}

pub fn draw_world_model_shaded(
    fb: &mut Framebuffer,
    model: &Model,
    camera: &FlightView,
    transform: WorldTransform,
    sun: Option<Vec3>,
    effect: crate::shaders::SurfaceEffect,
) {
    if !camera.sphere_visible(transform.position, transform.radius) {
        return;
    }
    let (sin, cos) = transform.yaw.sin_cos();
    let vertices: Vec<Vec3> = model
        .vertices
        .iter()
        .map(|vertex| {
            let p = (vertex - model.center) * (transform.radius / model.radius);
            let rotated = Vec3::new(p.x * cos + p.z * sin, p.y, p.z * cos - p.x * sin);
            camera.view_point(transform.position + rotated)
        })
        .collect();
    let light = sun
        .map(|position| camera.view_direction(position - transform.position))
        .filter(|direction| direction.norm_squared() > 0.001)
        .unwrap_or_else(|| Vec3::new(-0.4, 0.8, 1.0))
        .normalize();
    let half = (light + Vec3::new(0.0, 0.0, 1.0)).normalize();
    let mut primitives = Vec::with_capacity(model.faces.len());
    let cull_closed = matches!(
        effect,
        crate::shaders::SurfaceEffect::Kepler { .. }
            | crate::shaders::SurfaceEffect::Solar { .. }
            | crate::shaders::SurfaceEffect::Lava { .. }
    ) && camera.view_point(transform.position).norm_squared()
        > transform.radius * transform.radius;
    for face in &model.faces {
        let [i, j, k] = face.indices;
        let [a, b, c] = [vertices[i], vertices[j], vertices[k]];
        let normal = (b - a).cross(&(c - a));
        // Estas superficies son cerradas; las caras posteriores no necesitan ejecutar el shader.
        if cull_closed && normal.dot(&a) >= 0.0 {
            continue;
        }
        if let (Some(texture), Some(uv)) = (face.texture, face.uv) {
            let points = [a, b, c];
            let (polygon, count) = clip_near_textured(std::array::from_fn(|i| ClipVertex {
                point: points[i],
                uv: uv[i],
            }));
            let intensity = if sun.is_some() && !face.unlit && normal.norm_squared() > 0.001 {
                let normal = normal.normalize();
                let normal = if normal.z < 0.0 { -normal } else { normal };
                0.18 + 0.95 * normal.dot(&light).max(0.0)
            } else {
                texture_intensity(normal, face.unlit)
            };
            for index in 1..count.saturating_sub(1) {
                let projected = [polygon[0], polygon[index], polygon[index + 1]].map(|vertex| {
                    let mut point = vertex.point;
                    point.z = point.z.min(-NEAR_PLANE);
                    let screen = camera.project_view(point).unwrap();
                    triangle::TextureVertex {
                        screen,
                        uv: vertex.uv,
                        inverse_w: screen.z,
                    }
                });
                primitives.push(Primitive {
                    vertices: projected.map(|v| v.screen),
                    surface: Surface::Textured {
                        vertices: projected,
                        texture: &model.textures[texture],
                        tint: face.color,
                        intensity,
                        effect,
                    },
                });
            }
            continue;
        }
        let Some(color) = lit_color((b - a).cross(&(c - a)), face.color, &light, &half) else {
            continue;
        };
        let (polygon, count) = clip_near([a, b, c]);
        for index in 1..count.saturating_sub(1) {
            // Clipping guarantees positive depth; clamp roundoff at the near plane.
            let project = |mut p: Vec3| {
                p.z = p.z.min(-NEAR_PLANE);
                camera.project_view(p).unwrap()
            };
            let points = [polygon[0], polygon[index], polygon[index + 1]];
            let projected = points.map(project);
            let surface = if matches!(effect, crate::shaders::SurfaceEffect::Rock { .. }) {
                let local = points.map(|point| {
                    let world = camera.eye + camera.right * point.x + camera.up * point.y
                        - camera.forward * point.z;
                    let p = (world - transform.position) / transform.radius;
                    Vec3::new(p.x * cos - p.z * sin, p.y, p.x * sin + p.z * cos)
                });
                Surface::ProceduralColor {
                    local,
                    reciprocal: projected.map(|p| p.z),
                    color,
                    effect,
                }
            } else {
                Surface::Solid(color)
            };
            primitives.push(Primitive {
                vertices: projected,
                surface,
            });
        }
    }
    raster_batch::draw(fb, &primitives);
}

fn clip_near(triangle: [Vec3; 3]) -> ([Vec3; 4], usize) {
    let (polygon, count) = clip_near_textured(triangle.map(|point| ClipVertex {
        point,
        uv: Vec2::zeros(),
    }));
    (polygon.map(|v| v.point), count)
}

#[derive(Clone, Copy)]
struct ClipVertex {
    point: Vec3,
    uv: Vec2,
}

fn clip_near_textured(triangle: [ClipVertex; 3]) -> ([ClipVertex; 4], usize) {
    let mut output = [ClipVertex {
        point: Vec3::zeros(),
        uv: Vec2::zeros(),
    }; 4];
    let mut count = 0;
    let mut previous = triangle[2];
    for current in triangle {
        let was_inside = previous.point.z <= -NEAR_PLANE;
        let inside = current.point.z <= -NEAR_PLANE;
        if was_inside != inside {
            let t = (-NEAR_PLANE - previous.point.z) / (current.point.z - previous.point.z);
            output[count] = ClipVertex {
                point: previous.point + (current.point - previous.point) * t,
                uv: previous.uv + (current.uv - previous.uv) * t,
            };
            output[count].point.z = -NEAR_PLANE;
            count += 1;
        }
        if inside {
            output[count] = current;
            count += 1;
        }
        previous = current;
    }
    (output, count)
}

fn texture_intensity(normal: Vec3, unlit: bool) -> f32 {
    if unlit {
        return 1.0;
    }
    if normal.norm_squared() < f32::EPSILON {
        return 0.4;
    }
    let mut normal = normal.normalize();
    if normal.z < 0.0 {
        normal = -normal;
    }
    0.4 + 0.85 * normal.dot(&Vec3::new(-0.4, 0.8, 1.0).normalize()).max(0.0)
}

fn lit_color(normal: Vec3, base: Color, light: &Vec3, half: &Vec3) -> Option<Color> {
    if normal.norm_squared() < f32::EPSILON {
        return None;
    }
    let mut normal = normal.normalize();
    if normal.z < 0.0 {
        normal = -normal;
    }
    let diffuse = normal.dot(light).max(0.0);
    let highlight = normal.dot(half).max(0.0).powf(28.0) * 0.30;
    let surface = base.shaded(0.40 + 0.85 * diffuse);
    let rim = (1.0 - normal.z).powf(3.0) * 0.10;
    let lift = |channel: u8| (channel as f32 + 255.0 * (highlight + rim)).min(255.0) as u8;
    Some(Color::new(
        lift(surface.r),
        lift(surface.g),
        lift(surface.b),
    ))
}

/// Render the actual ship orientation in the same perspective as the star field.
pub fn draw_flight_model(
    framebuffer: &mut Framebuffer,
    model: &Model,
    flight: &Flight,
    fallback_color: Option<Color>,
    onboard: Option<&crate::lighting::ShipLights>,
) {
    draw_flight_model_lit(
        framebuffer,
        model,
        flight,
        fallback_color,
        None,
        None,
        onboard,
    );
}

pub fn draw_flight_model_lit(
    framebuffer: &mut Framebuffer,
    model: &Model,
    flight: &Flight,
    fallback_color: Option<Color>,
    solar: Option<&crate::lighting::SolarLight>,
    hull: Option<&crate::lighting::HullShadow>,
    onboard: Option<&crate::lighting::ShipLights>,
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
            let local = (vertex - model.center) * (crate::collision::SHIP_RADIUS / model.radius);
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
    if solar.is_some() || onboard.is_some() {
        let eye = flight.camera_position();
        let world: Vec<Vec3> = view
            .iter()
            .map(|v| eye + right * v.x + up * v.y - forward * v.z)
            .collect();
        let relative_sun = solar
            .map(|sun| (sun.position - flight.position) / crate::collision::SHIP_RADIUS)
            .unwrap_or_default();
        let local_sun = Vec3::new(
            -relative_sun.dot(&ship_right),
            relative_sun.dot(&ship_up),
            relative_sun.dot(&ship_forward),
        );
        let mut primitives = Vec::with_capacity(model.faces.len());
        for face in &model.faces {
            let [i, j, k] = face.indices;
            if [i, j, k].iter().any(|&index| view[index].z >= -0.1) {
                continue;
            }
            let normal = (world[j] - world[i]).cross(&(world[k] - world[i]));
            if normal.norm_squared() < 1e-12 {
                continue;
            }
            let normal = normal.normalize();
            let point = (world[i] + world[j] + world[k]) / 3.0;
            let facing = if normal.dot(&(eye - point)) < 0.0 {
                -normal
            } else {
                normal
            };
            // El rayo usa las coordenadas normalizadas del casco para reutilizar su arbol espacial.
            let local = ((model.vertices[i] + model.vertices[j] + model.vertices[k]) / 3.0
                - model.center)
                / model.radius;
            let blocked = solar.is_some_and(|sun| facing.dot(&(sun.position - point)) > 0.0)
                && hull.is_some_and(|hull| hull.occludes(local, local_sun));
            let mut color = solar
                .map(|sun| {
                    sun.shade(
                        point,
                        normal,
                        eye,
                        fallback_color.unwrap_or(face.color),
                        blocked,
                    )
                })
                .unwrap_or(Color::new(0, 0, 0));
            if let Some(onboard) = onboard {
                let local_direction =
                    |v: Vec3| Vec3::new(-v.dot(&ship_right), v.dot(&ship_up), v.dot(&ship_forward));
                let engine_power =
                    (flight.velocity.norm() / crate::flight::CRUISE_SPEED).clamp(0.0, 3.0);
                color = onboard.shade(
                    color,
                    fallback_color.unwrap_or(face.color),
                    local,
                    local_direction(normal),
                    local_direction((eye - point).normalize()),
                    engine_power,
                );
            }
            primitives.push(Primitive {
                vertices: [screen[i], screen[j], screen[k]],
                surface: Surface::Solid(color),
            });
        }
        raster_batch::draw(framebuffer, &primitives);
        return;
    }
    rasterize(
        framebuffer,
        model,
        &view,
        &screen,
        RenderMode::Solid,
        fallback_color,
        true,
    );
}

fn rasterize(
    framebuffer: &mut Framebuffer,
    model: &Model,
    view: &[Vec3],
    screen: &[Vec3],
    mode: RenderMode,
    fallback_color: Option<Color>,
    perspective: bool,
) {
    let light = Vec3::new(-0.4, 0.8, 1.0).normalize();
    let half = (light + Vec3::new(0.0, 0.0, 1.0)).normalize();
    for face in &model.faces {
        let [i, j, k] = face.indices;
        let [a, b, c] = [screen[i], screen[j], screen[k]];
        if mode == RenderMode::Solid {
            if fallback_color.is_none()
                && let (Some(texture), Some(uv)) = (face.texture, face.uv)
            {
                let points = [a, b, c];
                let vertices = std::array::from_fn(|index| triangle::TextureVertex {
                    screen: points[index],
                    uv: uv[index],
                    inverse_w: if perspective { points[index].z } else { 1.0 },
                });
                let intensity =
                    texture_intensity((view[j] - view[i]).cross(&(view[k] - view[i])), face.unlit);
                triangle::fill_textured(
                    framebuffer,
                    vertices,
                    &model.textures[texture],
                    face.color,
                    intensity,
                );
                continue;
            }
            let base = fallback_color.unwrap_or(face.color);
            if let Some(color) = lit_color(
                (view[j] - view[i]).cross(&(view[k] - view[i])),
                base,
                &light,
                &half,
            ) {
                triangle::fill(framebuffer, &a, &b, &c, color);
            }
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
    fn near_plane_clipping_preserves_uvs_at_new_vertices() {
        let (polygon, count) = clip_near_textured([
            ClipVertex {
                point: Vec3::new(0.0, 0.0, -0.5),
                uv: Vec2::new(0.0, 0.0),
            },
            ClipVertex {
                point: Vec3::new(1.0, 0.0, 0.5),
                uv: Vec2::new(1.0, 0.0),
            },
            ClipVertex {
                point: Vec3::new(0.0, 1.0, -0.5),
                uv: Vec2::new(0.0, 1.0),
            },
        ]);
        assert_eq!(count, 4);
        assert_eq!(polygon[1].uv, Vec2::new(0.25, 0.0));
        assert_eq!(polygon[2].uv, Vec2::new(0.25, 0.75));
    }

    #[test]
    fn triangles_crossing_the_camera_are_clipped_before_projection() {
        let (polygon, count) = clip_near([
            Vec3::new(-1.0, 0.0, 0.1),
            Vec3::new(1.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, -1.0),
        ]);
        assert_eq!(count, 4);
        assert!(
            polygon[..count]
                .iter()
                .all(|p| p.z <= -NEAR_PLANE && p.iter().all(|v| v.is_finite()))
        );
        assert_eq!(clip_near([Vec3::new(0.0, 0.0, 1.0); 3]).1, 0);
    }

    #[test]
    fn solar_ship_render_darkens_in_eclipse_without_changing_depth() {
        use crate::lighting::{Occluder, SolarLight};
        let flight = Flight::new();
        let sun = flight.position + Vec3::z() * 600.0 + Vec3::y() * 180.0;
        let light = SolarLight {
            position: sun,
            radius: 130.0,
            occluders: vec![],
        };
        let eclipse = SolarLight {
            position: sun,
            radius: 130.0,
            occluders: vec![Occluder {
                center: (flight.position + sun) * 0.5,
                radius: 120.0,
            }],
        };
        for ship in crate::fleet::load().unwrap() {
            let mut lit = Framebuffer::new(1280, 840);
            let mut dark = Framebuffer::new(1280, 840);
            draw_flight_model_lit(
                &mut lit,
                &ship.model,
                &flight,
                ship.finish,
                Some(&light),
                Some(&ship.shadows),
                None,
            );
            draw_flight_model_lit(
                &mut dark,
                &ship.model,
                &flight,
                ship.finish,
                Some(&eclipse),
                Some(&ship.shadows),
                None,
            );
            assert_eq!(lit.depth, dark.depth);
            let brightness = |fb: &Framebuffer| -> u64 {
                fb.pixels
                    .iter()
                    .zip(&fb.depth)
                    .filter(|(_, depth)| depth.is_finite())
                    .map(|(&p, _)| ((p >> 16 & 255) + (p >> 8 & 255) + (p & 255)) as u64)
                    .sum()
            };
            assert!(
                brightness(&lit) > brightness(&dark) * 2,
                "{} must darken during eclipse",
                ship.name
            );
            lit.depth.fill(f32::INFINITY);
            lit.pixels.fill(0x123456);
            draw_flight_model_lit(
                &mut lit,
                &ship.model,
                &flight,
                ship.finish,
                Some(&light),
                Some(&ship.shadows),
                None,
            );
            assert!(lit.pixels.iter().all(|&p| p == 0x123456));
        }
    }

    #[test]
    fn onboard_lights_keep_every_ship_visible_in_eclipse_and_without_a_sun() {
        use crate::lighting::{Occluder, SolarLight};
        let flight = Flight::new();
        let sun = flight.position + Vec3::z() * 600.0;
        let eclipse = SolarLight {
            position: sun,
            radius: 130.0,
            occluders: vec![Occluder {
                center: (flight.position + sun) * 0.5,
                radius: 120.0,
            }],
        };
        let clear = SolarLight {
            position: sun,
            radius: 130.0,
            occluders: vec![],
        };
        for ship in crate::fleet::load().unwrap() {
            let render = |sun, onboard| {
                let mut frame = Framebuffer::new(1280, 840);
                draw_flight_model_lit(
                    &mut frame,
                    &ship.model,
                    &flight,
                    ship.finish,
                    sun,
                    Some(&ship.shadows),
                    onboard,
                );
                frame
            };
            let unassisted = render(Some(&eclipse), None);
            let assisted = render(Some(&eclipse), Some(&ship.lights));
            let sunlit = render(Some(&clear), Some(&ship.lights));
            let dark_space = render(None, Some(&ship.lights));
            let brightness = |frame: &Framebuffer| -> f32 {
                let values: Vec<_> = frame
                    .pixels
                    .iter()
                    .zip(&frame.depth)
                    .filter(|(_, d)| d.is_finite())
                    .map(|(&p, _)| ((p >> 16 & 255) + (p >> 8 & 255) + (p & 255)) as f32 / 3.0)
                    .collect();
                values.iter().sum::<f32>() / values.len() as f32
            };
            assert_eq!(assisted.depth, unassisted.depth);
            assert_eq!(assisted.depth, dark_space.depth);
            assert!(
                brightness(&assisted) > brightness(&unassisted) * 3.0,
                "{}",
                ship.name
            );
            assert!(
                brightness(&assisted) > 20.0 && brightness(&dark_space) > 20.0,
                "{}",
                ship.name
            );
            assert!(
                brightness(&sunlit) > brightness(&assisted) * 1.1,
                "El eclipse debe seguir siendo visible: {}",
                ship.name
            );
        }
    }

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
                draw_flight_model(
                    &mut fb,
                    &ship.model,
                    &flight,
                    ship.finish,
                    Some(&ship.lights),
                );
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
