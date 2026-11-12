use crate::{
    color::Color,
    framebuffer::Framebuffer,
    noise,
    renderer::{FLIGHT_FOCAL, FlightView, WorldTransform},
};
use nalgebra_glm::{Vec2, Vec3};

// Quintic gradient noise for smooth plasma without value-noise square patches.
fn gradient_noise(p: Vec3) -> f32 {
    let cell = p.map(|v| v.floor() as i32);
    let f = p - p.map(f32::floor);
    let fade = f.map(|t| t * t * t * (t * (t * 6.0 - 15.0) + 10.0));
    let corner = |x: i32, y: i32, z: i32| {
        let h = (noise::hash(cell.x + x, cell.y + y, cell.z + z) * 255.0) as u32 & 15;
        let d = f - Vec3::new(x as f32, y as f32, z as f32);
        let u = if h < 8 { d.x } else { d.y };
        let v = if h < 4 {
            d.y
        } else if h == 12 || h == 14 {
            d.x
        } else {
            d.z
        };
        (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
    };
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let row = |y, z| mix(corner(0, y, z), corner(1, y, z), fade.x);
    (0.5 + 0.5
        * mix(
            mix(row(0, 0), row(1, 0), fade.y),
            mix(row(0, 1), row(1, 1), fade.y),
            fade.z,
        ))
    .clamp(0.0, 1.0)
}

// A solar-inspired visualization with accelerated time, not a plasma simulation.
pub fn photosphere(
    point: Vec3,
    seconds: f32,
    eye: Vec3,
    pixel_radius: f32,
    plasma: Option<f32>,
) -> Color {
    let p = if plasma.is_some() {
        point
    } else {
        point.normalize()
    };
    let view = (eye - point).normalize();
    let mu = p.dot(&view).max(0.0);
    // The atlas already contains the fine plasma detail. Avoid recalculating
    // procedural granulation on top of it at every pixel of a close-up.
    let (flow, granules, medium_weight, fine_weight) = if plasma.is_some() {
        (0.5, 0.5, 0.0, 0.0)
    } else {
        // The procedural fallback uses rotated gradient noise and footprint filtering.
        let q = Vec3::new(p.x * 0.8 + p.z * 0.6, p.y, p.z * 0.8 - p.x * 0.6);
        let q = Vec3::new(q.x, q.y * 0.8 - q.z * 0.6, q.y * 0.6 + q.z * 0.8);
        let flow = gradient_noise(q * 18.0 + Vec3::new(13.0, seconds * 0.035, 29.0));
        let footprint = 1.0 / (pixel_radius * mu.max(0.08)).max(1.0);
        let medium_weight = 1.0 - noise::smoothstep(0.25, 0.65, 18.0 * footprint);
        let fine_weight = 1.0 - noise::smoothstep(0.22, 0.55, 68.0 * footprint);
        let granules = if fine_weight > 0.0 {
            gradient_noise(q * 68.0 + Vec3::new(flow * 1.4, seconds * 0.12, flow * 0.7))
        } else {
            0.5
        };
        (flow, granules, medium_weight, fine_weight)
    };
    let mut spot = 0.0f32;
    let mut active = 0.0f32;
    for (index, direction) in [
        Vec3::new(0.30, 0.22, 0.93),
        Vec3::new(-0.55, -0.24, 0.80),
        Vec3::new(0.62, -0.45, -0.64),
    ]
    .into_iter()
    .enumerate()
    {
        let distance_squared = (p - direction.normalize()).norm_squared();
        // All spots and their bright surroundings fit within this angular cap.
        // Avoid sqrt, temporal trigonometry and spot noise on the rest of the disk.
        if distance_squared > 0.04 {
            continue;
        }
        let distance = distance_squared.sqrt();
        let radius = 0.047 + 0.008 * (seconds * 0.035 + index as f32 * 2.1).sin();
        let edge = if distance < radius * 2.0 {
            noise::noise(p * 38.0 + Vec3::new(9.0, seconds * 0.025, 17.0)) - 0.5
        } else {
            0.0
        };
        let distorted = distance + edge * 0.025;
        let umbra = 1.0 - noise::smoothstep(radius * 0.50, radius, distorted);
        let penumbra = 1.0 - noise::smoothstep(radius, radius * 1.65, distorted);
        spot = spot.max(umbra * 0.42 + penumbra * 0.18);
        let plage =
            (1.0 - noise::smoothstep(radius * 1.4, radius * 3.2, distance)) * (1.0 - penumbra);
        active = active.max(plage);
    }
    let brightness =
        (0.93 + (granules - 0.5) * 0.045 * fine_weight + (flow - 0.5) * 0.055 * medium_weight)
            * (0.56 + 0.44 * mu.sqrt())
            * (1.0 - spot);
    let faculae = if active > 0.0 {
        active * (0.035 + 0.025 * (seconds * 0.32 + flow * 5.0).sin()) * (0.4 + 0.6 * (1.0 - mu))
    } else {
        0.0
    };
    let emission = if let Some(heat) = plasma {
        let heat = noise::smoothstep(0.28, 0.90, heat);
        Vec3::new(160.0, 193.0, 223.0) + Vec3::new(95.0, 68.0, 50.0) * heat
    } else {
        Vec3::new(255.0, 261.0, 273.0)
    };
    let rgb = emission * brightness + Vec3::new(212.0, 234.0, 255.0) * faculae;
    Color::new(
        rgb.x.clamp(0.0, 255.0) as u8,
        rgb.y.clamp(0.0, 255.0) as u8,
        rgb.z.clamp(0.0, 255.0) as u8,
    )
}

/// High-resolution plasma uses the same bilinear sampler as Aurora, with its
/// own flow, white-blue thermal palette, spots, limb darkening and corona.
pub fn sample(
    texture: &crate::texture::Texture,
    uv: Vec2,
    seconds: f32,
    eye: Vec3,
    pixel_radius: f32,
) -> Color {
    let (point, pole_weight) = crate::shaders::sphere_direction(uv);
    let flow = noise::noise(point * 5.0 + Vec3::new(7.0, seconds * 0.055, 13.0));
    let coordinates = Vec2::new(
        (uv.x + seconds * 0.0017 + (flow - 0.5) * 0.04 * pole_weight).rem_euclid(1.0),
        (uv.y + (flow - 0.5) * 0.018 * pole_weight).clamp(0.0, 1.0),
    );
    let base = texture.sample(coordinates);
    let heat = (base.r as f32 * 0.20 + base.g as f32 * 0.55 + base.b as f32 * 0.25) / 255.0;
    photosphere(point, seconds, eye, pixel_radius, Some(heat))
}

/// Emission outside the Blender sphere; this creates no geometry or collision mesh.
pub fn draw_corona(fb: &mut Framebuffer, view: &FlightView, star: WorldTransform, seconds: f32) {
    let Some(center) = view.project(star.position) else {
        return;
    };
    let local_center = view.view_point(star.position);
    let depth = -local_center.z;
    let relative = star.position - view.eye();
    let distance_to_center = relative.norm();
    if distance_to_center <= star.radius * 1.01 {
        return;
    }
    let outer = star.radius * 1.52;
    let bounds = if depth > outer {
        // Conservative perspective bounds also cover an off-axis sphere.
        let reach_x = FLIGHT_FOCAL * outer * (1.0 + local_center.x.abs() / depth) / (depth - outer);
        let reach_y = FLIGHT_FOCAL * outer * (1.0 + local_center.y.abs() / depth) / (depth - outer);
        (
            (center.x - reach_x).max(0.0) as usize,
            (center.y - reach_y).max(0.0) as usize,
            (center.x + reach_x).ceil().clamp(0.0, fb.width as f32) as usize,
            (center.y + reach_y).ceil().clamp(0.0, fb.height as f32) as usize,
        )
    } else {
        (0, 0, fb.width, fb.height)
    };
    let center_direction = relative / distance_to_center;
    let (camera_right, _, forward) = view.basis();
    let right = (camera_right - center_direction * camera_right.dot(&center_direction)).normalize();
    let up = right.cross(&center_direction);
    let angular_scale = distance_to_center / star.radius;
    let (sin, cos) = star.yaw.sin_cos();
    let loops = [
        Vec3::new(-0.98, 0.17, 0.05),
        Vec3::new(0.70, 0.70, 0.10),
        Vec3::new(0.10, -0.98, -0.05),
    ]
    .map(|p| {
        let p = Vec3::new(p.x * cos + p.z * sin, p.y, p.z * cos - p.x * sin).normalize();
        let x = p.dot(&right);
        let y = -p.dot(&up);
        let length = (x * x + y * y).sqrt().max(0.0001);
        (
            x / length,
            y / length,
            1.0 - noise::smoothstep(0.15, 0.45, p.dot(&center_direction).abs()),
        )
    });
    fb.shade_rect(bounds, |x, y, pixel, stored_depth| {
        // Angular impact matches the sphere silhouette from any camera angle.
        let ray = view.ray(x as f32 + 0.5, y as f32 + 0.5).normalize();
        let along = relative.dot(&ray);
        if along <= 0.0 || 1.0 / (along * ray.dot(&forward)) <= *stored_depth {
            return;
        }
        let dx = ray.dot(&right) * angular_scale;
        let dy = -ray.dot(&up) * angular_scale;
        let distance = (dx * dx + dy * dy).sqrt();
        if !(0.995..1.52).contains(&distance) {
            return;
        }
        let direction = Vec3::new(dx / distance, dy / distance, seconds * 0.035);
        let turbulence = noise::noise(
            direction.component_mul(&Vec3::new(8.0, 8.0, 1.0)) + Vec3::new(3.0, 7.0, 11.0),
        );
        let altitude = distance - 1.0;
        let chromosphere = (-altitude / 0.012).exp() * 0.30;
        let corona = (-altitude / (0.075 + turbulence * 0.075)).exp() * (0.04 + turbulence * 0.10);
        let mut prominence = 0.0;
        for (index, (ax, ay, visible)) in loops.iter().enumerate() {
            if *visible <= 0.0 {
                continue;
            }
            let phase = (seconds / 22.0 + index as f32 * 0.31).rem_euclid(1.0);
            let envelope = (std::f32::consts::PI * phase).sin().powi(2);
            let height = 0.045 + phase * 0.14;
            let tangent = -dx * ay + dy * ax;
            let radial = dx * ax + dy * ay;
            let ring = ((tangent / 0.085).powi(2) + ((radial - 1.015) / height).powi(2)).sqrt();
            let ring = ring
                + (turbulence - 0.5) * 0.22
                + (radial * 50.0 + tangent * 25.0 - seconds * 0.4).sin() * 0.04;
            let width = 0.05 + 0.025 * turbulence;
            let filament = (-((ring - 1.0) / width).powi(2)).exp();
            let glow = (-((ring - 1.0) / (width * 3.0)).powi(2)).exp() * 0.18;
            prominence += (filament + glow) * envelope * visible * (0.18 + turbulence * 0.18);
        }
        *pixel = crate::framebuffer::emission_color(
            *pixel,
            0xc7deff,
            chromosphere + corona + prominence,
            0.0,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::Flight;

    #[test]
    fn photosphere_is_emissive_has_a_darker_limb_and_evolves_reproducibly() {
        let eye = Vec3::z() * 5.0;
        let center = photosphere(Vec3::z(), 0.0, eye, 512.0, None);
        let limb = photosphere(
            Vec3::new(0.97, 0.0, 0.243).normalize(),
            0.0,
            eye,
            512.0,
            None,
        );
        assert!(center.r > limb.r + 25);
        assert!(center.b >= center.r && center.r > 180);
        let mut changed = 0;
        for index in 0..64 {
            let p = Vec3::new(
                (index as f32 * 0.7).sin() * 0.7,
                (index as f32 * 1.3).cos() * 0.6,
                1.0,
            )
            .normalize();
            let a = photosphere(p, 0.0, eye, 512.0, None).to_hex();
            assert_eq!(a, photosphere(p, 0.0, eye, 512.0, None).to_hex());
            if a != photosphere(p, 6.0, eye, 512.0, None).to_hex() {
                changed += 1;
            }
        }
        assert!(
            changed > 40,
            "La granulacion debe evolucionar con el tiempo"
        );
    }

    #[test]
    fn corona_touches_the_true_silhouette_when_star_is_off_axis() {
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        let view = FlightView::new(&flight, 320, 240);
        let star = WorldTransform {
            position: Vec3::new(100.0, 0.0, -320.0),
            radius: 65.0,
            yaw: 0.0,
        };
        let mut frame = Framebuffer::new(320, 240);
        frame.pixels.fill(0);
        draw_corona(&mut frame, &view, star, 0.0);
        let relative = star.position - view.eye();
        let mut samples = 0;
        for y in 0..240 {
            for x in 0..320 {
                let ray = view.ray(x as f32 + 0.5, y as f32 + 0.5).normalize();
                let impact = (relative - ray * relative.dot(&ray)).norm() / star.radius;
                if (1.004..1.015).contains(&impact) {
                    samples += 1;
                    assert!(
                        (frame.pixels[y * 320 + x] & 255) > 30,
                        "La corona no debe separarse del limbo en ({x}, {y})"
                    );
                }
            }
        }
        assert!(samples > 20);
    }

    #[test]
    fn corona_animates_without_writing_depth_or_covering_foreground_objects() {
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        let view = FlightView::new(&flight, 320, 240);
        let star = WorldTransform {
            position: Vec3::new(0.0, 0.0, -400.0),
            radius: 45.0,
            yaw: 0.0,
        };
        let render = |seconds| {
            let mut frame = Framebuffer::new(320, 240);
            frame.pixels.fill(0);
            let before = frame.depth.clone();
            draw_corona(&mut frame, &view, star, seconds);
            assert_eq!(frame.depth, before);
            frame
        };
        let a = render(0.0);
        let b = render(6.0);
        assert!(a.pixels.iter().filter(|p| **p != 0).count() > 100);
        assert!(
            a.pixels
                .iter()
                .zip(&b.pixels)
                .filter(|(a, b)| a != b)
                .count()
                > 100
        );
        let mut blocked = Framebuffer::new(320, 240);
        blocked.pixels.fill(0xff00ff);
        blocked.depth.fill(1.0);
        draw_corona(&mut blocked, &view, star, 6.0);
        assert!(blocked.pixels.iter().all(|p| *p == 0xff00ff));
        assert!(blocked.depth.iter().all(|z| *z == 1.0));
    }
}
