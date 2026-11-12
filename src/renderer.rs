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
    framebuffer.triangles_processed += model.faces.len();
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

#[derive(Clone, Copy)]
pub struct FlightView {
    eye: Vec3,
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    width: f32,
    height: f32,
    principal: Vec2,
    exit_clip: Option<(Vec3, Vec3)>,
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
            principal: Vec2::new(width as f32 * 0.5, height as f32 * 0.5),
            exit_clip: None,
        }
    }

    pub fn eye(&self) -> Vec3 {
        self.eye
    }

    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        (self.right, self.up, self.forward)
    }

    pub fn projection_center(&self) -> Vec2 {
        self.principal
    }

    pub fn looking_at(&self, eye: Vec3, target: Vec3) -> Self {
        let forward = (target - eye).try_normalize(1e-6).unwrap_or(self.forward);
        let reference = if forward.y.abs() > 0.99 {
            Vec3::z()
        } else {
            Vec3::y()
        };
        let right = forward.cross(&reference).normalize();
        Self {
            eye,
            right,
            up: right.cross(&forward),
            forward,
            exit_clip: None,
            ..*self
        }
    }

    pub fn ray(&self, x: f32, y: f32) -> Vec3 {
        self.forward
            + self.right * ((x - self.principal.x) / FLIGHT_FOCAL)
            + self.up * ((self.principal.y - y) / FLIGHT_FOCAL)
    }

    pub fn through(&self, entry: crate::portal::Portal, exit: crate::portal::Portal) -> Self {
        let side = if (self.eye - entry.center).dot(&entry.normal) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let normal = exit.normal * side;
        Self {
            eye: entry.point_to(exit, self.eye),
            right: entry.rotate_to(exit, self.right),
            up: entry.rotate_to(exit, self.up),
            forward: entry.rotate_to(exit, self.forward),
            exit_clip: Some((exit.center + normal * 0.015, normal)),
            ..*self
        }
    }

    /// Preserve the full-screen rays in a small off-axis portal render target.
    pub fn cropped(&self, bounds: (usize, usize, usize, usize)) -> Self {
        Self {
            width: (bounds.2 - bounds.0) as f32,
            height: (bounds.3 - bounds.1) as f32,
            principal: self.principal - Vec2::new(bounds.0 as f32, bounds.1 as f32),
            ..*self
        }
    }

    pub fn contains_world(&self, point: Vec3) -> bool {
        self.exit_clip
            .is_none_or(|(origin, normal)| (point - origin).dot(&normal) >= 0.0)
    }

    fn clip_triangle(&self, triangle: [ClipVertex; 3]) -> ([ClipVertex; 5], usize) {
        let (near, count) = clip_near_textured(triangle);
        let mut result = [near[0]; 5];
        let Some((origin, normal)) = self.exit_clip else {
            result[..count].copy_from_slice(&near[..count]);
            return (result, count);
        };
        if count == 0 {
            return (result, 0);
        }
        let normal = self.view_direction(normal);
        let origin = self.view_point(origin);
        let distance = |v: ClipVertex| (v.point - origin).dot(&normal);
        let mut result_count = 0;
        let mut previous = near[count - 1];
        for &current in &near[..count] {
            let (before, now) = (distance(previous), distance(current));
            if (before >= 0.0) != (now >= 0.0) {
                let t = before / (before - now);
                result[result_count] = ClipVertex {
                    point: previous.point + (current.point - previous.point) * t,
                    uv: previous.uv + (current.uv - previous.uv) * t,
                };
                result_count += 1;
            }
            if now >= 0.0 {
                result[result_count] = current;
                result_count += 1;
            }
            previous = current;
        }
        (result, result_count)
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
                self.principal.x + point.x * FLIGHT_FOCAL / depth,
                self.principal.y - point.y * FLIGHT_FOCAL / depth,
                1.0 / depth,
            )
        })
    }

    pub fn project(&self, world: Vec3) -> Option<Vec3> {
        if !self.contains_world(world) {
            return None;
        }
        self.project_view(self.view_point(world))
    }

    pub fn sphere_visible(&self, center: Vec3, radius: f32) -> bool {
        if self
            .exit_clip
            .is_some_and(|(origin, normal)| (center - origin).dot(&normal) < -radius)
        {
            return false;
        }
        let point = self.view_point(center);
        let depth = -point.z;
        let left = self.principal.x / FLIGHT_FOCAL;
        let right = (self.width - self.principal.x) / FLIGHT_FOCAL;
        let top = self.principal.y / FLIGHT_FOCAL;
        let bottom = (self.height - self.principal.y) / FLIGHT_FOCAL;
        depth + radius >= NEAR_PLANE
            && point.x >= -depth * left - radius * (1.0 + left * left).sqrt()
            && point.x <= depth * right + radius * (1.0 + right * right).sqrt()
            && point.y <= depth * top + radius * (1.0 + top * top).sqrt()
            && point.y >= -depth * bottom - radius * (1.0 + bottom * bottom).sqrt()
    }
}

/// Opaque world objects and the ship share reciprocal depth in the framebuffer.
#[cfg(test)]
pub fn draw_world_model(
    fb: &mut Framebuffer,
    model: &Model,
    camera: &FlightView,
    transform: WorldTransform,
) {
    draw_world_model_lit(fb, model, camera, transform, None);
}

#[cfg(test)]
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
    draw_world_with_basis(fb, model, camera, transform, sun, effect, None, None);
}

pub fn draw_ship_in_view(
    fb: &mut Framebuffer,
    ship: &crate::fleet::Ship,
    flight: &Flight,
    camera: &FlightView,
    sun: Option<Vec3>,
) {
    let (right, up, forward) = flight.ship_basis();
    draw_world_with_basis(
        fb,
        &ship.model,
        camera,
        WorldTransform {
            position: flight.position,
            radius: crate::collision::SHIP_RADIUS,
            yaw: 0.0,
        },
        sun,
        crate::shaders::SurfaceEffect::None,
        Some([-right, up, forward]),
        ship.finish,
    );
}

fn draw_world_with_basis(
    fb: &mut Framebuffer,
    model: &Model,
    camera: &FlightView,
    transform: WorldTransform,
    sun: Option<Vec3>,
    effect: crate::shaders::SurfaceEffect,
    basis: Option<[Vec3; 3]>,
    finish: Option<Color>,
) {
    if fb.render_options.culling && !camera.sphere_visible(transform.position, transform.radius) {
        return;
    }
    // Se cuentan las caras que entran al pipeline, antes del descarte posterior y del recorte.
    fb.triangles_processed += model.faces.len();
    let (sin, cos) = transform.yaw.sin_cos();
    let vertices: Vec<Vec3> = model
        .vertices
        .iter()
        .map(|vertex| {
            let p = (vertex - model.center) * (transform.radius / model.radius);
            let rotated = basis.map_or_else(
                || Vec3::new(p.x * cos + p.z * sin, p.y, p.z * cos - p.x * sin),
                |[right, up, forward]| right * p.x + up * p.y + forward * p.z,
            );
            camera.view_point(transform.position + rotated)
        })
        .collect();
    let light = sun
        .map(|position| camera.view_direction(position - transform.position))
        .filter(|direction| direction.norm_squared() > 0.001)
        .unwrap_or_else(|| Vec3::new(-0.4, 0.8, 1.0))
        .normalize();
    let half = (light + Vec3::new(0.0, 0.0, 1.0)).normalize();
    let local_vector = |p: Vec3| Vec3::new(p.x * cos - p.z * sin, p.y, p.x * sin + p.z * cos);
    let world_light = camera.right * light.x + camera.up * light.y - camera.forward * light.z;
    let effect = match effect {
        crate::shaders::SurfaceEffect::GasGiant { seconds, .. } => {
            crate::shaders::SurfaceEffect::GasGiant {
                seconds,
                light: local_vector(world_light.normalize()),
                eye: local_vector((camera.eye - transform.position).normalize()),
            }
        }
        crate::shaders::SurfaceEffect::Stellar { seconds, .. } => {
            crate::shaders::SurfaceEffect::Stellar {
                seconds,
                eye: local_vector((camera.eye - transform.position) / transform.radius),
                pixel_radius: transform.radius * FLIGHT_FOCAL
                    / (camera.eye - transform.position).norm().max(0.1),
            }
        }
        crate::shaders::SurfaceEffect::Ice {
            seconds,
            core_radius,
            ..
        } => crate::shaders::SurfaceEffect::Ice {
            seconds,
            core_radius,
            light: local_vector(
                (sun.unwrap_or(transform.position + world_light * 10000.0) - transform.position)
                    / transform.radius,
            ),
            eye: local_vector((camera.eye - transform.position) / transform.radius),
            normal: Vec3::y(),
            detail: transform.radius * FLIGHT_FOCAL
                / (camera.eye - transform.position).norm().max(0.1)
                >= 96.0,
        },
        _ => effect,
    };
    // Onice keeps its modeled craters and palette; interpolate normals and material colors.
    let smooth_surface = if matches!(effect, crate::shaders::SurfaceEffect::Obsidian) {
        let mut normals = vec![Vec3::zeros(); vertices.len()];
        let mut colors = vec![Vec3::zeros(); vertices.len()];
        let mut areas = vec![0.0; vertices.len()];
        for face in &model.faces {
            let [i, j, k] = face.indices;
            let normal = (vertices[j] - vertices[i]).cross(&(vertices[k] - vertices[i]));
            let area = normal.norm();
            let color = Vec3::new(
                face.color.r as f32,
                face.color.g as f32,
                face.color.b as f32,
            );
            for index in face.indices {
                normals[index] += normal;
                colors[index] += color * area;
                areas[index] += area;
            }
        }
        for index in 0..normals.len() {
            normals[index] = normals[index].try_normalize(1e-10).unwrap_or(Vec3::z());
            colors[index] /= areas[index].max(1e-10);
        }
        Some((normals, colors))
    } else {
        None
    };
    let mut primitives = Vec::with_capacity(model.faces.len());
    let cull_closed = matches!(
        effect,
        crate::shaders::SurfaceEffect::Kepler { .. }
            | crate::shaders::SurfaceEffect::Solar { .. }
            | crate::shaders::SurfaceEffect::Stellar { .. }
            | crate::shaders::SurfaceEffect::GoldenStar { .. }
            | crate::shaders::SurfaceEffect::Asteroid
            | crate::shaders::SurfaceEffect::Ice { .. }
            | crate::shaders::SurfaceEffect::Obsidian
            | crate::shaders::SurfaceEffect::Snow { .. }
            | crate::shaders::SurfaceEffect::PackIce { .. }
            | crate::shaders::SurfaceEffect::GasGiant { .. }
            | crate::shaders::SurfaceEffect::Lava { .. }
            | crate::shaders::SurfaceEffect::CrimsonLava { .. }
    ) && camera.view_point(transform.position).norm_squared()
        > transform.radius * transform.radius;
    for face in &model.faces {
        let [i, j, k] = face.indices;
        let [a, b, c] = [vertices[i], vertices[j], vertices[k]];
        let normal = (b - a).cross(&(c - a));
        let effect = if let crate::shaders::SurfaceEffect::Ice {
            seconds,
            light,
            eye,
            detail,
            core_radius,
            ..
        } = effect
        {
            if normal.norm_squared() < 1e-12 {
                continue;
            }
            let world_normal =
                camera.right * normal.x + camera.up * normal.y - camera.forward * normal.z;
            crate::shaders::SurfaceEffect::Ice {
                seconds,
                light,
                eye,
                detail,
                core_radius,
                normal: local_vector(world_normal.normalize()),
            }
        } else {
            effect
        };
        // Estas superficies son cerradas; las caras posteriores no necesitan ejecutar el shader.
        if cull_closed && normal.dot(&a) >= 0.0 {
            continue;
        }
        if let (Some(texture), Some(uv)) = (face.texture, face.uv) {
            let points = [a, b, c];
            let (polygon, count) = camera.clip_triangle(std::array::from_fn(|i| ClipVertex {
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
                    opacity: face.opacity,
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
        let color = if face.unlit
            || matches!(
                effect,
                crate::shaders::SurfaceEffect::Ice { .. } | crate::shaders::SurfaceEffect::Obsidian
            ) {
            Some(finish.unwrap_or(face.color))
        } else if matches!(effect, crate::shaders::SurfaceEffect::Asteroid) {
            let diffuse = normal.normalize().dot(&light).max(0.0);
            Some(face.color.shaded(0.14 + diffuse * 0.90))
        } else {
            lit_color(
                (b - a).cross(&(c - a)),
                finish.unwrap_or(face.color),
                &light,
                &half,
            )
        };
        let Some(color) = color else {
            continue;
        };
        let (polygon, count) = camera.clip_triangle([a, b, c].map(|point| ClipVertex {
            point,
            uv: Vec2::zeros(),
        }));
        for index in 1..count.saturating_sub(1) {
            // Clipping guarantees positive depth; clamp roundoff at the near plane.
            let project = |mut p: Vec3| {
                p.z = p.z.min(-NEAR_PLANE);
                camera.project_view(p).unwrap()
            };
            let points = [
                polygon[0].point,
                polygon[index].point,
                polygon[index + 1].point,
            ];
            let projected = points.map(project);
            let surface = if matches!(
                effect,
                crate::shaders::SurfaceEffect::Rock { .. }
                    | crate::shaders::SurfaceEffect::Lava { .. }
                    | crate::shaders::SurfaceEffect::CrimsonLava { .. }
                    | crate::shaders::SurfaceEffect::Ice { .. }
                    | crate::shaders::SurfaceEffect::Stellar { .. }
                    | crate::shaders::SurfaceEffect::GoldenStar { .. }
                    | crate::shaders::SurfaceEffect::Asteroid
                    | crate::shaders::SurfaceEffect::Obsidian
                    | crate::shaders::SurfaceEffect::Snow { .. }
                    | crate::shaders::SurfaceEffect::Crystal
                    | crate::shaders::SurfaceEffect::PackIce { .. }
                    | crate::shaders::SurfaceEffect::GasGiant { .. }
                    | crate::shaders::SurfaceEffect::RingIce
            ) {
                let local = points.map(|point| {
                    let world = camera.eye + camera.right * point.x + camera.up * point.y
                        - camera.forward * point.z;
                    let p = (world - transform.position) / transform.radius;
                    Vec3::new(p.x * cos - p.z * sin, p.y, p.x * sin + p.z * cos)
                });
                let lighting = smooth_surface.as_ref().map(|(normals, colors)| {
                    let u = b - a;
                    let v = c - a;
                    let determinant = u.norm_squared() * v.norm_squared() - u.dot(&v).powi(2);
                    let at = |point: Vec3, values: &[Vec3]| {
                        if determinant <= 1e-12 {
                            return values[i];
                        }
                        let delta = point - a;
                        let w1 = (delta.dot(&u) * v.norm_squared() - delta.dot(&v) * u.dot(&v))
                            / determinant;
                        let w2 = (delta.dot(&v) * u.norm_squared() - delta.dot(&u) * u.dot(&v))
                            / determinant;
                        values[i] * (1.0 - w1 - w2) + values[j] * w1 + values[k] * w2
                    };
                    raster_batch::SmoothLighting {
                        normals: points.map(|point| at(point, normals)),
                        colors: points.map(|point| at(point, colors)),
                        light,
                        half,
                    }
                });
                Surface::ProceduralColor {
                    local,
                    reciprocal: projected.map(|p| p.z),
                    color,
                    effect,
                    lighting,
                }
            } else {
                Surface::Solid(color)
            };
            primitives.push(Primitive {
                vertices: projected,
                surface,
                opacity: face.opacity,
            });
        }
    }
    raster_batch::draw(fb, &primitives);
}

#[cfg(test)]
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

pub(crate) fn lit_color(normal: Vec3, base: Color, light: &Vec3, half: &Vec3) -> Option<Color> {
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
    framebuffer.triangles_processed += model.faces.len();
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
            let blocked = face.opacity >= 1.0
                && solar.is_some_and(|sun| facing.dot(&(sun.position - point)) > 0.0)
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
                opacity: face.opacity,
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
    let mut primitives = Vec::with_capacity(model.faces.len());
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
                primitives.push(Primitive {
                    vertices: points,
                    surface: Surface::Textured {
                        vertices,
                        texture: &model.textures[texture],
                        tint: face.color,
                        intensity,
                        effect: crate::shaders::SurfaceEffect::None,
                    },
                    opacity: face.opacity,
                });
                continue;
            }
            let base = fallback_color.unwrap_or(face.color);
            if let Some(color) = lit_color(
                (view[j] - view[i]).cross(&(view[k] - view[i])),
                base,
                &light,
                &half,
            ) {
                primitives.push(Primitive {
                    vertices: [a, b, c],
                    surface: Surface::Solid(color),
                    opacity: face.opacity,
                });
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
    raster_batch::draw(framebuffer, &primitives);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_counter_tracks_culling_without_changing_the_image_and_resets_per_frame() {
        let model = Model::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/uv-checker.obj"
        ))
        .unwrap();
        let flight = Flight::new();
        let view = FlightView::new(&flight, 256, 192);
        let transform = WorldTransform {
            position: Vec3::new(10000.0, 0.0, -100.0),
            radius: 1.0,
            yaw: 0.0,
        };
        let mut culled = Framebuffer::new(256, 192);
        let mut full = Framebuffer::new(256, 192);
        full.render_options.culling = false;
        draw_world_model(&mut culled, &model, &view, transform);
        draw_world_model(&mut full, &model, &view, transform);
        assert_eq!(culled.pixels, full.pixels);
        assert_eq!(culled.depth, full.depth);
        assert_eq!(culled.triangles_processed, 0);
        assert_eq!(full.triangles_processed, model.faces.len());
        full.clear();
        assert_eq!(full.triangles_processed, 0);
        assert!(!full.render_options.culling);
    }

    #[test]
    fn portal_camera_preserves_projection_parallax_and_oblique_orientation() {
        use crate::portal::Portal;
        let mut flight = Flight::new();
        flight.position = Vec3::new(2.0, 3.0, 4.0);
        let source = FlightView::new(&flight, 1280, 840);
        let entry = Portal::new(Vec3::new(0.0, 0.0, -20.0), Vec3::z()).unwrap();
        let exit = Portal::new(Vec3::new(100.0, 40.0, -200.0), Vec3::new(1.0, 0.4, -0.2)).unwrap();
        let remote = source.through(entry, exit);
        let point = entry.center - entry.normal * 10.0 + entry.right * 2.0 + entry.up;
        let expected = source.project(point).unwrap();
        let actual = remote.project(entry.point_to(exit, point)).unwrap();
        assert!((expected - actual).norm() < 0.001);
        assert!((exit.point_to(entry, remote.eye()) - source.eye()).norm() < 1e-4);
        assert!(
            (remote.ray(713.5, 382.5) - entry.rotate_to(exit, source.ray(713.5, 382.5))).norm()
                < 1e-5
        );
        let cropped = remote.cropped((400, 200, 850, 650));
        assert!((cropped.ray(52.5, 71.5) - remote.ray(452.5, 271.5)).norm() < 1e-6);
        let moved = FlightView {
            eye: source.eye + Vec3::x(),
            ..source
        }
        .through(entry, exit);
        assert!((moved.eye() - remote.eye()).norm() > 0.99);
        assert_ne!(moved.project(entry.point_to(exit, point)), Some(actual));
    }

    #[test]
    fn exit_plane_clips_triangles_and_interpolates_uvs_instead_of_showing_objects_before_exit() {
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        let mut view = FlightView::new(&flight, 320, 240);
        view.exit_clip = Some((Vec3::new(0.0, 0.0, -5.0), -Vec3::z()));
        let world = [
            Vec3::new(-1.0, 0.0, -4.0),
            Vec3::new(1.0, 0.0, -6.0),
            Vec3::new(0.0, 2.0, -6.0),
        ];
        let uv = [Vec2::zeros(), Vec2::x(), Vec2::y()];
        let (polygon, count) = view.clip_triangle(std::array::from_fn(|i| ClipVertex {
            point: view.view_point(world[i]),
            uv: uv[i],
        }));
        assert_eq!(count, 4);
        for v in &polygon[..count] {
            let p =
                view.eye + view.right * v.point.x + view.up * v.point.y - view.forward * v.point.z;
            assert!(p.z <= -5.0 + 1e-5);
        }
        assert!(
            polygon[..count]
                .iter()
                .any(|v| (v.uv - Vec2::new(0.5, 0.0)).norm() < 1e-5)
        );
        assert!(!view.sphere_visible(Vec3::zeros(), 1.0));
        assert!(view.project(Vec3::zeros()).is_none());
        assert!(view.project(Vec3::new(0.0, 0.0, -10.0)).is_some());
    }

    #[test]
    fn looking_through_the_back_face_keeps_the_opposite_exit_half_space() {
        use crate::portal::Portal;
        let entry = Portal::new(Vec3::zeros(), Vec3::z()).unwrap();
        let exit = Portal::new(Vec3::new(100.0, 40.0, 0.0), Vec3::x()).unwrap();
        let mut flight = Flight::new();
        flight.camera.yaw += std::f32::consts::PI;
        let view = FlightView {
            eye: -Vec3::z() * 20.0,
            ..FlightView::new(&flight, 320, 240)
        }
        .through(entry, exit);
        assert!(view.contains_world(exit.center - exit.normal * 10.0));
        assert!(!view.contains_world(exit.center + exit.normal * 10.0));
        assert!(view.project(exit.center - exit.normal * 10.0).is_some());
    }

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
    fn benatar_chase_view_is_large_enough_to_read_the_hull() {
        let ships = crate::fleet::load().unwrap();
        let ship = &ships[0];
        let flight = Flight::new();
        let mut frame = Framebuffer::new(1280, 840);
        draw_flight_model(
            &mut frame,
            &ship.model,
            &flight,
            ship.finish,
            Some(&ship.lights),
        );
        let xs: Vec<_> = frame
            .pixels
            .iter()
            .enumerate()
            .filter(|(_, p)| **p != crate::framebuffer::BACKGROUND)
            .map(|(i, _)| i % frame.width)
            .collect();
        let width = xs.iter().max().unwrap() - xs.iter().min().unwrap();
        assert!(
            width >= 330,
            "La Benatar debe verse cercana en vuelo: ancho {width} px"
        );
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
                            (350..930).contains(&x) && (440..790).contains(&y),
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
