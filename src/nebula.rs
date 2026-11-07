use crate::noise::{fbm, hash, noise, smoothstep};
use nalgebra_glm::Vec3;
use rayon::prelude::*;
use std::sync::OnceLock;

fn pack(c: Vec3) -> u32 {
    ((c.x.clamp(0.0, 255.0) as u32) << 16)
        | ((c.y.clamp(0.0, 255.0) as u32) << 8)
        | c.z.clamp(0.0, 255.0) as u32
}

fn unpack(c: u32) -> Vec3 {
    Vec3::new(
        ((c >> 16) & 255) as f32,
        ((c >> 8) & 255) as f32,
        (c & 255) as f32,
    )
}

/// Fondo procedural fijo en pantalla. Se calcula una sola vez, sin leer imágenes.
pub fn generate(width: usize, height: usize) -> Vec<u32> {
    let mut pixels = vec![0; width * height];
    let extent = height.max(1) as f32;
    if width == 0 || height == 0 {
        return pixels;
    }
    pixels
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, pixel) in row.iter_mut().enumerate() {
                let p = Vec3::new(
                    (x as f32 - width as f32 * 0.5) / extent,
                    (y as f32 - height as f32 * 0.5) / extent,
                    0.73,
                );
                // La franja galáctica atraviesa la imagen de abajo-izquierda a arriba-derecha.
                let warp = noise(p * 3.2 + Vec3::new(7.0, 2.0, 5.0)) - 0.5;
                let distance = p.y + p.x * 0.55 - 0.04 + warp * 0.18;
                let band = (-distance * distance * 19.0).exp();
                let clouds = fbm(p * 6.0 + Vec3::new(15.0, 3.0, -8.0));
                let wisps = fbm(p * 19.0 + Vec3::new(4.0, 23.0, 6.0));
                let density = smoothstep(0.28, 0.76, clouds) * band;
                let hue = smoothstep(-0.55, 0.6, p.x + (clouds - 0.5) * 1.6);
                let violet = Vec3::new(71.0, 30.0, 108.0);
                let cyan = Vec3::new(20.0, 79.0, 103.0);
                let nebula = violet + (cyan - violet) * hue;
                // Polvo oscuro dentro de las nubes rompe la uniformidad del resplandor.
                let dust = smoothstep(0.44, 0.67, wisps) * band;
                let mut color = Vec3::new(2.0, 3.0, 9.0) + nebula * density * (0.4 + wisps * 0.9);
                color += Vec3::new(23.0, 27.0, 43.0) * band * clouds * 0.3;
                color *= 1.0 - dust * 0.72;
                let vignette = (1.0 - (p.x * p.x + p.y * p.y) * 0.22).clamp(0.55, 1.0);
                *pixel = pack(color * vignette);
            }
        });
    // Semilla fija: cada estrella conserva su posición al cambiar de cuerpo o tiempo.
    let count = width * height / 700;
    for i in 0..count {
        let seed = i as i32;
        let x = hash(seed, 17, 43) * width.saturating_sub(1) as f32;
        let y = hash(seed, 59, 71) * height.saturating_sub(1) as f32;
        let class = hash(seed, 83, 97);
        let bright = class > 0.986;
        let radius = if bright {
            1.45 + hash(seed, 11, 13) * 0.65
        } else {
            0.48 + hash(seed, 11, 13) * 0.5
        };
        let temperature = hash(seed, 101, 103);
        let color = if temperature < 0.25 {
            Vec3::new(159.0, 193.0, 255.0)
        } else if temperature > 0.86 {
            Vec3::new(255.0, 209.0, 158.0)
        } else {
            Vec3::new(229.0, 234.0, 255.0)
        };
        let energy = if bright {
            1.2
        } else {
            0.3 + hash(seed, 107, 109) * 0.8
        };
        let reach = if bright { 11 } else { 3 };
        for py in (y as i32 - reach).max(0)..=(y as i32 + reach).min(height as i32 - 1) {
            for px in (x as i32 - reach).max(0)..=(x as i32 + reach).min(width as i32 - 1) {
                let dx = px as f32 + 0.5 - x;
                let dy = py as f32 + 0.5 - y;
                let d2 = dx * dx + dy * dy;
                // Núcleo suave subpíxel; halo y destellos solo en las estrellas más brillantes.
                let mut intensity = (-d2 / (radius * radius * 0.5)).exp();
                if bright {
                    intensity += 0.14 * (-d2 / (radius * radius * 8.0)).exp();
                    intensity += 0.20
                        * ((-dx.abs() * 0.45 - dy.abs() * 5.0).exp()
                            + (-dy.abs() * 0.45 - dx.abs() * 5.0).exp());
                }
                let index = py as usize * width + px as usize;
                pixels[index] = pack(unpack(pixels[index]) + color * intensity * energy);
            }
        }
    }
    pixels
}

pub struct Backgrounds {
    pub hangar: Vec<u32>,
    pub destinations: Vec<u32>,
}

pub fn backgrounds() -> &'static Backgrounds {
    static CACHE: OnceLock<Backgrounds> = OnceLock::new();
    CACHE.get_or_init(|| {
        let hangar = generate(crate::ui::WIDTH, crate::ui::HEIGHT);
        // La variacion mantiene las estrellas y cambia el color de las nubes.
        let destinations = hangar
            .par_iter()
            .map(|&pixel| {
                let color = unpack(pixel);
                let neutral = color.x.min(color.y).min(color.z);
                let cloud = color - Vec3::repeat(neutral);
                pack(
                    Vec3::repeat(neutral)
                        + Vec3::new(
                            cloud.x * 1.4 + cloud.z * 0.24,
                            cloud.y * 0.58,
                            cloud.z * 0.78 + cloud.x * 0.2,
                        ),
                )
            })
            .collect();
        Backgrounds {
            hangar,
            destinations,
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn cosmos_is_repeatable_and_contains_dark_regions_clouds_and_sparse_bright_stars() {
        let pixels = generate(320, 240);
        assert_eq!(pixels.len(), 320 * 240);
        assert_eq!(pixels, generate(320, 240));
        assert!(pixels.iter().copied().collect::<HashSet<_>>().len() > 200);
        let brightness = |p: u32| ((p >> 16) & 255).max((p >> 8) & 255).max(p & 255);
        let dark = pixels.iter().filter(|p| brightness(**p) < 80).count();
        let bright = pixels.iter().filter(|p| brightness(**p) > 180).count();
        assert!(dark > pixels.len() * 3 / 4);
        assert!(
            bright > 10 && bright < pixels.len() / 50,
            "{bright} estrellas brillantes"
        );
    }

    #[test]
    fn small_and_non_square_frames_have_valid_pixel_counts() {
        for (width, height) in [(1, 1), (7, 19), (200, 80)] {
            assert_eq!(generate(width, height).len(), width * height);
        }
    }
}
