use crate::{
    camera::Camera,
    framebuffer::Framebuffer,
    noise::{fbm, hash, noise, smoothstep},
    renderer::FLIGHT_FOCAL,
    ui::mix,
};
use nalgebra_glm::Vec3;
use rayon::prelude::*;
use std::sync::OnceLock;

const SIZE: usize = 512;
type Cubemap = [Vec<u32>; 6];

fn face_direction(face: usize, u: f32, v: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, v, -u),
        1 => Vec3::new(-1.0, v, u),
        2 => Vec3::new(u, 1.0, -v),
        3 => Vec3::new(u, -1.0, v),
        4 => Vec3::new(u, v, 1.0),
        _ => Vec3::new(-u, v, -1.0),
    }
    .normalize()
}

fn sky_color(direction: Vec3) -> u32 {
    let warp = noise(direction * 3.2 + Vec3::new(7.0, 2.0, 5.0)) - 0.5;
    let plane = direction.y * 0.82 + direction.x * 0.40 + direction.z * 0.18 + warp * 0.35;
    let band = (-plane * plane * 7.5).exp();
    let clouds = fbm(direction * 5.5 + Vec3::new(15.0, 3.0, -8.0));
    let wisps = fbm(direction * 19.0 + Vec3::new(4.0, 23.0, 6.0));
    let density = smoothstep(0.24, 0.74, clouds) * band;
    let hue = smoothstep(-0.6, 0.6, direction.x + (clouds - 0.5) * 1.8);
    let violet = Vec3::new(56.0, 18.0, 82.0);
    let blue = Vec3::new(9.0, 30.0, 67.0);
    let nebula = violet + (blue - violet) * hue;
    let dust = smoothstep(0.46, 0.69, wisps) * band;
    let mut color = Vec3::new(2.0, 2.0, 8.0) + nebula * density * (0.5 + wisps * 1.05);
    color += Vec3::new(10.0, 12.0, 28.0) * band * clouds * 0.25;
    color *= 1.0 - dust * 0.75;
    let cell = (direction * 430.0).map(|v| v.floor() as i32);
    let star = hash(cell.x, cell.y, cell.z);
    let brightness = ((star - 0.9985) / 0.0015).max(0.0).powi(2) * 220.0;
    color += Vec3::new(0.84, 0.9, 1.0) * brightness;
    let color = color.map(|channel| channel.clamp(0.0, 255.0).round() as u32);
    color.x << 16 | color.y << 8 | color.z
}

fn cubemap() -> &'static Cubemap {
    static CACHE: OnceLock<Cubemap> = OnceLock::new();
    CACHE.get_or_init(|| {
        std::array::from_fn(|face| {
            let mut pixels = vec![0; SIZE * SIZE];
            pixels
                .par_chunks_mut(SIZE)
                .enumerate()
                .for_each(|(y, row)| {
                    for (x, pixel) in row.iter_mut().enumerate() {
                        let u = x as f32 / (SIZE - 1) as f32 * 2.0 - 1.0;
                        let v = y as f32 / (SIZE - 1) as f32 * 2.0 - 1.0;
                        *pixel = sky_color(face_direction(face, u, v));
                    }
                });
            pixels
        })
    })
}

pub fn prepare() {
    let _ = cubemap();
}

fn sample(cube: &Cubemap, direction: Vec3) -> u32 {
    let absolute = direction.map(f32::abs);
    let (face, u, v) = if absolute.x >= absolute.y && absolute.x >= absolute.z {
        if direction.x >= 0.0 {
            (0, -direction.z / absolute.x, direction.y / absolute.x)
        } else {
            (1, direction.z / absolute.x, direction.y / absolute.x)
        }
    } else if absolute.y >= absolute.z {
        if direction.y >= 0.0 {
            (2, direction.x / absolute.y, -direction.z / absolute.y)
        } else {
            (3, direction.x / absolute.y, direction.z / absolute.y)
        }
    } else if direction.z >= 0.0 {
        (4, direction.x / absolute.z, direction.y / absolute.z)
    } else {
        (5, -direction.x / absolute.z, direction.y / absolute.z)
    };
    let x = ((u + 1.0) * 0.5).clamp(0.0, 1.0) * (SIZE - 1) as f32;
    let y = ((v + 1.0) * 0.5).clamp(0.0, 1.0) * (SIZE - 1) as f32;
    let (left, top) = (x.floor() as usize, y.floor() as usize);
    let right = (left + 1).min(SIZE - 1);
    let bottom = (top + 1).min(SIZE - 1);
    let row_a = mix(
        cube[face][top * SIZE + left],
        cube[face][top * SIZE + right],
        x - left as f32,
    );
    let row_b = mix(
        cube[face][bottom * SIZE + left],
        cube[face][bottom * SIZE + right],
        x - left as f32,
    );
    mix(row_a, row_b, y - top as f32)
}

pub fn draw(fb: &mut Framebuffer, camera: &Camera) {
    let cube = cubemap();
    let (right, up, forward) = camera.basis();
    let width = fb.width;
    let height = fb.height;
    // El cielo depende de la orientacion, permanece en el infinito y no escribe profundidad.
    fb.pixels
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, row)| {
            let vertical = (height as f32 * 0.5 - y as f32 - 0.5) / FLIGHT_FOCAL;
            let base = forward + up * vertical;
            for (x, pixel) in row.iter_mut().enumerate() {
                let horizontal = (x as f32 + 0.5 - width as f32 * 0.5) / FLIGHT_FOCAL;
                *pixel = sample(cube, base + right * horizontal);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skybox_closes_cube_edges_rotates_with_camera_and_preserves_depth() {
        let cube = cubemap();
        for y in 0..SIZE {
            assert_eq!(cube[0][y * SIZE + SIZE - 1], cube[5][y * SIZE]);
        }
        let mut a = Framebuffer::new(320, 210);
        let mut b = Framebuffer::new(320, 210);
        let mut camera = Camera::new();
        draw(&mut a, &camera);
        draw(&mut b, &camera);
        assert_eq!(a.pixels, b.pixels);
        assert!(a.depth.iter().all(|&depth| depth == f32::NEG_INFINITY));
        camera.orbit(0.5, 0.1);
        draw(&mut b, &camera);
        assert_ne!(a.pixels, b.pixels);
        let colorful = cube
            .iter()
            .flatten()
            .filter(|&&p| ((p >> 16) & 255) > 15 || ((p >> 8) & 255) > 15)
            .count();
        assert!(colorful > SIZE * SIZE / 3);
    }
}
