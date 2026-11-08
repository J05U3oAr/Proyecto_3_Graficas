use crate::{color::Color, framebuffer::Framebuffer, texture::Texture, triangle::TextureVertex};
use nalgebra_glm::Vec3;
use rayon::prelude::*;

const BAND_HEIGHT: usize = 32;

pub enum Surface<'a> {
    Solid(Color),
    ProceduralColor {
        local: [Vec3; 3],
        reciprocal: [f32; 3],
        color: Color,
        effect: crate::shaders::SurfaceEffect,
    },
    Textured {
        vertices: [TextureVertex; 3],
        texture: &'a Texture,
        tint: Color,
        intensity: f32,
        effect: crate::shaders::SurfaceEffect,
    },
}

pub struct Primitive<'a> {
    pub vertices: [Vec3; 3],
    pub surface: Surface<'a>,
}

fn edge(a: Vec3, b: Vec3, x: f32, y: f32) -> f32 {
    (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
}

impl Primitive<'_> {
    fn color(&self, weights: [f32; 3]) -> u32 {
        match &self.surface {
            Surface::Solid(color) => color.to_hex(),
            Surface::ProceduralColor {
                local,
                reciprocal,
                color,
                effect,
            } => {
                let depth = reciprocal[0] * weights[0]
                    + reciprocal[1] * weights[1]
                    + reciprocal[2] * weights[2];
                let point = (local[0] * (weights[0] * reciprocal[0])
                    + local[1] * (weights[1] * reciprocal[1])
                    + local[2] * (weights[2] * reciprocal[2]))
                    / depth;
                effect.modulate(*color, point).to_hex()
            }
            Surface::Textured {
                vertices: [a, b, c],
                texture,
                tint,
                intensity,
                effect,
            } => {
                let [w0, w1, w2] = weights;
                let reciprocal = a.inverse_w * w0 + b.inverse_w * w1 + c.inverse_w * w2;
                let uv = (a.uv * (w0 * a.inverse_w)
                    + b.uv * (w1 * b.inverse_w)
                    + c.uv * (w2 * c.inverse_w))
                    / reciprocal;
                let color = effect.sample(texture, uv);
                let channel = |value: u8, tint: u8| {
                    (value as f32 * tint as f32 / 255.0 * intensity)
                        .clamp(0.0, 255.0)
                        .round() as u8
                };
                Color::new(
                    channel(color.r, tint.r),
                    channel(color.g, tint.g),
                    channel(color.b, tint.b),
                )
                .to_hex()
            }
        }
    }
}

struct Prepared<'a, 'b> {
    primitive: &'a Primitive<'b>,
    area: f32,
    bounds: (usize, usize, usize, usize),
}

pub fn draw(fb: &mut Framebuffer, primitives: &[Primitive<'_>]) {
    let (left, top, right, bottom) = fb.clip;
    let prepared: Vec<_> = primitives
        .iter()
        .filter_map(|primitive| {
            let [a, b, c] = primitive.vertices;
            let area = edge(a, b, c.x, c.y);
            if area.abs() < 0.00001 {
                return None;
            }
            let x0 = a.x.min(b.x).min(c.x).floor().max(left as f32) as usize;
            let x1 = (a.x.max(b.x).max(c.x).ceil() + 1.0)
                .max(0.0)
                .min(right as f32) as usize;
            let y0 = a.y.min(b.y).min(c.y).floor().max(top as f32) as usize;
            let y1 = (a.y.max(b.y).max(c.y).ceil() + 1.0)
                .max(0.0)
                .min(bottom as f32) as usize;
            (x0 < x1 && y0 < y1).then_some(Prepared {
                primitive,
                area,
                bounds: (x0, y0, x1, y1),
            })
        })
        .collect();
    let mut bins = vec![Vec::new(); fb.height.div_ceil(BAND_HEIGHT)];
    // Cada franja conserva el orden de los triángulos y escribe en memoria exclusiva.
    for (index, item) in prepared.iter().enumerate() {
        for bin in &mut bins[item.bounds.1 / BAND_HEIGHT..=(item.bounds.3 - 1) / BAND_HEIGHT] {
            bin.push(index);
        }
    }
    let width = fb.width;
    let paint = |(band, (pixels, depth)): (usize, (&mut [u32], &mut [f32]))| {
        let band_y = band * BAND_HEIGHT;
        let end_y = band_y + pixels.len() / width;
        for &index in &bins[band] {
            let item = &prepared[index];
            let [a, b, c] = item.primitive.vertices;
            for y in item.bounds.1.max(band_y)..item.bounds.3.min(end_y) {
                for x in item.bounds.0..item.bounds.2 {
                    let px = x as f32 + 0.5;
                    let py = y as f32 + 0.5;
                    let w0 = edge(b, c, px, py) / item.area;
                    let w1 = edge(c, a, px, py) / item.area;
                    let w2 = edge(a, b, px, py) / item.area;
                    if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                        let z = w0 * a.z + w1 * b.z + w2 * c.z;
                        let offset = (y - band_y) * width + x;
                        // Las superficies ocultas se descartan antes de muestrear la textura.
                        if z > depth[offset] {
                            depth[offset] = z;
                            pixels[offset] = item.primitive.color([w0, w1, w2]);
                        }
                    }
                }
            }
        }
    };
    let chunk = width * BAND_HEIGHT;
    if prepared.len() >= 128 && rayon::current_num_threads() > 1 {
        fb.pixels
            .par_chunks_mut(chunk)
            .zip(fb.depth.par_chunks_mut(chunk))
            .enumerate()
            .for_each(paint);
    } else {
        fb.pixels
            .chunks_mut(chunk)
            .zip(fb.depth.chunks_mut(chunk))
            .enumerate()
            .for_each(paint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parallel_bands_match_reference_with_textures_clipping_and_depth() {
        let texture = Texture::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/uv-checker.png"
        ))
        .unwrap();
        let vertices = [
            TextureVertex {
                screen: Vec3::new(-8.0, 2.0, 1.0),
                uv: nalgebra_glm::Vec2::new(0.0, 0.0),
                inverse_w: 1.0,
            },
            TextureVertex {
                screen: Vec3::new(90.0, 5.0, 0.3),
                uv: nalgebra_glm::Vec2::new(1.0, 0.0),
                inverse_w: 0.3,
            },
            TextureVertex {
                screen: Vec3::new(35.0, 97.0, 0.5),
                uv: nalgebra_glm::Vec2::new(0.0, 1.0),
                inverse_w: 0.5,
            },
        ];
        let tint = Color::new(220, 240, 255);
        let primitives: Vec<_> = (0..140)
            .map(|_| Primitive {
                vertices: vertices.map(|v| v.screen),
                surface: Surface::Textured {
                    vertices,
                    texture: &texture,
                    tint,
                    intensity: 0.8,
                    effect: crate::shaders::SurfaceEffect::None,
                },
            })
            .collect();
        let mut reference = Framebuffer::new(96, 96);
        reference.set_clip(7, 3, 72, 80);
        reference.point(30, 30, 2.0, 0xff00ff, true);
        for _ in &primitives {
            crate::triangle::fill_textured(&mut reference, vertices, &texture, tint, 0.8);
        }
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut frame = Framebuffer::new(96, 96);
            frame.set_clip(7, 3, 72, 80);
            frame.point(30, 30, 2.0, 0xff00ff, true);
            pool.install(|| draw(&mut frame, &primitives));
            assert_eq!(frame.pixels, reference.pixels);
            assert_eq!(frame.depth, reference.depth);
        }
    }
}
