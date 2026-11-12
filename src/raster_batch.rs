use crate::{color::Color, framebuffer::Framebuffer, texture::Texture, triangle::TextureVertex};
use nalgebra_glm::{Vec2, Vec3};
use rayon::prelude::*;

const BAND_HEIGHT: usize = 32;

pub struct SmoothLighting {
    pub normals: [Vec3; 3],
    pub colors: [Vec3; 3],
    pub light: Vec3,
    pub half: Vec3,
}

pub enum Surface<'a> {
    Solid(Color),
    ProceduralColor {
        local: [Vec3; 3],
        reciprocal: [f32; 3],
        color: Color,
        effect: crate::shaders::SurfaceEffect,
        lighting: Option<SmoothLighting>,
    },
    Crystal {
        local: [Vec3; 3],
        reciprocal: [f32; 3],
        material: crate::crystal::Material,
        optics: crate::crystal::Optics,
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
    pub opacity: f32,
}

fn edge(a: Vec3, b: Vec3, x: f32, y: f32) -> f32 {
    (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
}

impl Primitive<'_> {
    fn optical(&self, weights: [f32; 3]) -> Option<crate::crystal::Fragment> {
        let Surface::Crystal {
            local,
            reciprocal,
            material,
            optics,
        } = &self.surface
        else {
            return None;
        };
        let w = std::array::from_fn::<_, 3, _>(|i| weights[i] * reciprocal[i]);
        let point = (local[0] * w[0] + local[1] * w[1] + local[2] * w[2]) / (w[0] + w[1] + w[2]);
        Some(crate::crystal::shade(*material, *optics, point))
    }

    fn color(&self, weights: [f32; 3]) -> u32 {
        match &self.surface {
            Surface::Solid(color) => color.to_hex(),
            Surface::Crystal { .. } => {
                crate::crystal::display(self.optical(weights).unwrap().radiance).to_hex()
            }
            Surface::ProceduralColor {
                local,
                reciprocal,
                color,
                effect,
                lighting,
            } => {
                let depth = reciprocal[0] * weights[0]
                    + reciprocal[1] * weights[1]
                    + reciprocal[2] * weights[2];
                let point = (local[0] * (weights[0] * reciprocal[0])
                    + local[1] * (weights[1] * reciprocal[1])
                    + local[2] * (weights[2] * reciprocal[2]))
                    / depth;
                let color = if let Some(lighting) = lighting {
                    let normal = (lighting.normals[0] * (weights[0] * reciprocal[0])
                        + lighting.normals[1] * (weights[1] * reciprocal[1])
                        + lighting.normals[2] * (weights[2] * reciprocal[2]))
                        / depth;
                    let rgb = (lighting.colors[0] * (weights[0] * reciprocal[0])
                        + lighting.colors[1] * (weights[1] * reciprocal[1])
                        + lighting.colors[2] * (weights[2] * reciprocal[2]))
                        / depth;
                    let base = Color::new(
                        rgb.x.clamp(0.0, 255.0) as u8,
                        rgb.y.clamp(0.0, 255.0) as u8,
                        rgb.z.clamp(0.0, 255.0) as u8,
                    );
                    if matches!(effect, crate::shaders::SurfaceEffect::PackIce { .. }) {
                        // Classify glacial albedo before lighting, so shadowed snow
                        // is never mistaken for the dark animated channels.
                        return crate::glacier::lighting(
                            normal,
                            effect.modulate(base, point),
                            &lighting.light,
                            &lighting.half,
                        )
                        .to_hex();
                    }
                    if matches!(effect, crate::shaders::SurfaceEffect::Snow { .. }) {
                        crate::shaders::snow_lighting(normal, base, &lighting.light, &lighting.half)
                    } else {
                        crate::renderer::lit_color(normal, base, &lighting.light, &lighting.half)
                            .unwrap_or(base)
                    }
                } else {
                    *color
                };
                effect.modulate(color, point).to_hex()
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

// Frozen opaque buffers prevent feedback and cross-band races during refraction.
struct Scene {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
    depth: Vec<f32>,
}

impl Scene {
    fn sample(&self, x: usize, y: usize, delta: Vec2, surface_depth: f32) -> Vec3 {
        let original = crate::crystal::unpack(self.pixels[y * self.width + x]);
        let point = Vec2::new(x as f32, y as f32) + delta;
        if point.x < 0.0
            || point.y < 0.0
            || point.x > (self.width - 1) as f32
            || point.y > (self.height - 1) as f32
        {
            return original;
        }
        let (left, top) = (point.x.floor() as usize, point.y.floor() as usize);
        let right = (left + 1).min(self.width - 1);
        let bottom = (top + 1).min(self.height - 1);
        let indices = [
            top * self.width + left,
            top * self.width + right,
            bottom * self.width + left,
            bottom * self.width + right,
        ];
        // A nearer ship or object must never be pulled into the glass silhouette.
        if indices.iter().any(|i| self.depth[*i] >= surface_depth) {
            return original;
        }
        let [a, b, c, d] = indices.map(|i| crate::crystal::unpack(self.pixels[i]));
        let u = point.x - left as f32;
        let v = point.y - top as f32;
        (a + (b - a) * u) * (1.0 - v) + (c + (d - c) * u) * v
    }
}

pub fn draw(fb: &mut Framebuffer, primitives: &[Primitive<'_>]) {
    let (left, top, right, bottom) = fb.clip;
    let mut prepared: Vec<_> = primitives
        .iter()
        .filter(|primitive| primitive.opacity > 0.0)
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
    // Opaque geometry establishes depth first. Glass blends from far to near
    // without hiding the cockpit or changing the opaque depth buffer.
    prepared.sort_by(|a, b| {
        let transparent_a = a.primitive.opacity < 1.0;
        let transparent_b = b.primitive.opacity < 1.0;
        transparent_a.cmp(&transparent_b).then_with(|| {
            if transparent_a && transparent_b {
                let depth = |item: &Prepared<'_, '_>| {
                    item.primitive.vertices.iter().map(|v| v.z).sum::<f32>()
                };
                depth(a).total_cmp(&depth(b))
            } else {
                std::cmp::Ordering::Equal
            }
        })
    });
    let refractive = prepared.iter().any(|item| {
        matches!(&item.primitive.surface,
        Surface::Crystal { material, .. } if material.kind == crate::crystal::Kind::Glass)
    });
    if refractive {
        let split = prepared
            .iter()
            .position(|p| p.primitive.opacity < 1.0)
            .unwrap_or(prepared.len());
        paint(fb, &prepared[..split], None);
        let scene = Scene {
            width: fb.width,
            height: fb.height,
            pixels: fb.pixels.clone(),
            depth: fb.depth.clone(),
        };
        paint(fb, &prepared[split..], Some(&scene));
    } else {
        paint(fb, &prepared, None);
    }
}

fn paint(fb: &mut Framebuffer, prepared: &[Prepared<'_, '_>], scene: Option<&Scene>) {
    if prepared.is_empty() {
        return;
    }
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
                        let opacity = item.primitive.opacity;
                        if opacity < 1.0 {
                            // Half-open edges avoid blending twice along a glass seam.
                            let inclusive = |mut from: Vec3, mut to: Vec3| {
                                if item.area < 0.0 {
                                    std::mem::swap(&mut from, &mut to);
                                }
                                to.y < from.y || (to.y == from.y && to.x > from.x)
                            };
                            if (w0 == 0.0 && !inclusive(b, c))
                                || (w1 == 0.0 && !inclusive(c, a))
                                || (w2 == 0.0 && !inclusive(a, b))
                            {
                                continue;
                            }
                        }
                        let z = w0 * a.z + w1 * b.z + w2 * c.z;
                        let offset = (y - band_y) * width + x;
                        // Las superficies ocultas se descartan antes de muestrear la textura.
                        if z > depth[offset] {
                            if let (Some(scene), Surface::Crystal { material, .. }) =
                                (scene, &item.primitive.surface)
                                && material.kind == crate::crystal::Kind::Glass
                            {
                                let fragment = item.primitive.optical([w0, w1, w2]).unwrap();
                                let delta = fragment.refraction.map(|v| v.clamp(-12.0, 12.0));
                                // Small spectral separation, without artificial rainbow bands.
                                let red = scene.sample(x, y, delta * 1.015, z).x;
                                let green = scene.sample(x, y, delta, z).y;
                                let blue = scene.sample(x, y, delta * 0.985, z).z;
                                let refracted = Vec3::new(red, green, blue);
                                let original = crate::crystal::unpack(scene.pixels[y * width + x]);
                                let previous = crate::crystal::unpack(pixels[offset]);
                                // Keep prior glass layers; displace only the frozen opaque scene.
                                let behind =
                                    (previous + refracted - original).map(|v| v.clamp(0.0, 255.0));
                                pixels[offset] = crate::crystal::display(
                                    fragment.radiance
                                        + fragment.transmission.component_mul(&behind),
                                )
                                .to_hex();
                                continue;
                            }
                            let color = item.primitive.color([w0, w1, w2]);
                            if opacity >= 1.0 {
                                depth[offset] = z;
                                pixels[offset] = color;
                            } else {
                                let blend = |shift: u32| {
                                    let from = ((pixels[offset] >> shift) & 255) as f32;
                                    let to = ((color >> shift) & 255) as f32;
                                    (from + (to - from) * opacity).round() as u32
                                };
                                pixels[offset] = blend(16) << 16 | blend(8) << 8 | blend(0);
                            }
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
    fn refractive_glass_matches_separate_passes_and_parallel_bands() {
        let quad = [
            Vec3::new(5.0, 5.0, 1.0),
            Vec3::new(59.0, 5.0, 1.0),
            Vec3::new(59.0, 59.0, 1.0),
            Vec3::new(5.0, 59.0, 1.0),
        ];
        let local = [
            Vec3::new(-0.3, 0.3, 0.0),
            Vec3::new(0.3, 0.3, 0.0),
            Vec3::new(0.3, -0.3, 0.0),
            Vec3::new(-0.3, -0.3, 0.0),
        ];
        let optics = crate::crystal::Optics {
            eye: Vec3::z() * 3.0,
            light: Vec3::new(-10.0, 10.0, 3.0),
            normal: Vec3::new(0.5, 0.0, 0.8660254),
            screen_right: Vec3::x(),
            screen_up: Vec3::y(),
            pixel_scale: 200.0,
            ground_radius: 0.1,
            sun_cosine: 0.99,
            rotation: Vec2::new(0.0, 1.0),
        };
        let make_glass = |scale| {
            [[0, 1, 2], [0, 2, 3]].map(|indices| Primitive {
                vertices: indices.map(|i| quad[i]),
                opacity: 0.34,
                surface: Surface::Crystal {
                    local: indices.map(|i| local[i]),
                    reciprocal: [1.0; 3],
                    material: crate::crystal::Material::new(Color::new(180, 210, 235), 0.34),
                    optics: crate::crystal::Optics {
                        pixel_scale: scale,
                        ..optics
                    },
                },
            })
        };
        let glass = make_glass(200.0);
        let make_opaque = || Primitive {
            vertices: [
                Vec3::new(28.0, 0.0, 0.4),
                Vec3::new(40.0, 0.0, 0.4),
                Vec3::new(34.0, 64.0, 0.4),
            ],
            opacity: 1.0,
            surface: Surface::Solid(Color::new(40, 220, 60)),
        };
        let opaque = make_opaque();
        let initial = || {
            let mut fb = Framebuffer::new(64, 64);
            for y in 0..64 {
                for x in 0..64 {
                    fb.pixels[y * 64 + x] = if x % 8 < 4 { 0x224488 } else { 0x99bbdd };
                }
            }
            fb.depth.fill(0.1);
            fb.point(35, 30, 3.0, 0xff00ff, true);
            fb
        };
        let mut separate = initial();
        draw(&mut separate, std::slice::from_ref(&opaque));
        let depth = separate.depth.clone();
        draw(&mut separate, &glass);
        assert_eq!(separate.depth, depth);
        assert_eq!(separate.pixels[30 * 64 + 35], 0xff00ff);
        assert_eq!(
            separate.pixels.iter().filter(|p| **p == 0xff00ff).count(),
            1,
            "La nave en primer plano no debe filtrarse a otros pixeles por refraccion"
        );
        let [a, b] = glass;
        let primitives = [opaque, a, b];
        let mut together = initial();
        draw(&mut together, &primitives);
        assert_eq!(together.pixels, separate.pixels);
        assert_eq!(together.depth, separate.depth);
        let mut straight = initial();
        draw(&mut straight, std::slice::from_ref(&make_opaque()));
        draw(&mut straight, &make_glass(0.0));
        assert!(
            straight
                .pixels
                .iter()
                .zip(&together.pixels)
                .filter(|(a, b)| a != b)
                .count()
                > 100,
            "La refraccion debe desplazar el patron del fondo"
        );
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut result = initial();
            // Hidden glass adds enough work to activate parallel bands without
            // changing the expected image. The visible glass runs in that pass.
            let mut parallel: Vec<_> = (0..128)
                .map(|_| Primitive {
                    vertices: [
                        Vec3::new(0.0, 0.0, 0.01),
                        Vec3::new(64.0, 0.0, 0.01),
                        Vec3::new(0.0, 64.0, 0.01),
                    ],
                    opacity: 0.34,
                    surface: Surface::Crystal {
                        local: [Vec3::zeros(); 3],
                        reciprocal: [0.01; 3],
                        material: crate::crystal::Material::new(Color::new(180, 210, 235), 0.34),
                        optics,
                    },
                })
                .collect();
            parallel.push(make_opaque());
            parallel.extend(make_glass(200.0));
            pool.install(|| draw(&mut result, &parallel));
            assert_eq!(result.pixels, together.pixels);
            assert_eq!(result.depth, together.depth);
        }
    }

    #[test]
    fn smooth_material_and_lighting_do_not_reveal_the_quad_diagonal() {
        let points = [
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(7.0, 1.0, 1.0),
            Vec3::new(7.0, 7.0, 1.0),
            Vec3::new(1.0, 7.0, 1.0),
        ];
        let colors = [
            Vec3::new(40.0, 60.0, 80.0),
            Vec3::new(100.0, 90.0, 80.0),
            Vec3::new(100.0, 150.0, 140.0),
            Vec3::new(40.0, 120.0, 140.0),
        ];
        let normals = [
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.4, 0.0, 1.0),
            Vec3::new(0.4, 0.4, 1.0),
            Vec3::new(0.0, 0.4, 1.0),
        ];
        let render = |triangles: [[usize; 3]; 2]| {
            let primitives = triangles.map(|indices| Primitive {
                vertices: indices.map(|i| points[i]),
                opacity: 1.0,
                surface: Surface::ProceduralColor {
                    local: [Vec3::zeros(); 3],
                    reciprocal: [1.0; 3],
                    color: Color::new(0, 0, 0),
                    effect: crate::shaders::SurfaceEffect::None,
                    lighting: Some(SmoothLighting {
                        normals: indices.map(|i| normals[i]),
                        colors: indices.map(|i| colors[i]),
                        light: Vec3::z(),
                        half: Vec3::z(),
                    }),
                },
            });
            let mut frame = Framebuffer::new(8, 8);
            frame.point(4, 4, 2.0, 0xff00ff, true);
            draw(&mut frame, &primitives);
            frame
        };
        let a = render([[0, 1, 2], [0, 2, 3]]);
        let b = render([[0, 1, 3], [1, 2, 3]]);
        for (a, b) in a.depth.iter().zip(&b.depth) {
            assert!(a == b || (a - b).abs() < 0.000001);
        }
        assert_eq!(a.pixels[4 * 8 + 4], 0xff00ff);
        for (a, b) in a.pixels.iter().zip(&b.pixels) {
            for shift in [0, 8, 16] {
                assert!(
                    (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs() <= 2,
                    "La iluminacion y el material deben continuar a traves de la diagonal: {a:06x} / {b:06x}"
                );
            }
        }
    }

    #[test]
    fn glass_sorts_layers_keeps_opaque_depth_and_blends_shared_edges_once() {
        let quad = |z, color, opacity| {
            let points = [
                Vec3::new(0.0, 0.0, z),
                Vec3::new(8.0, 0.0, z),
                Vec3::new(8.0, 8.0, z),
                Vec3::new(0.0, 8.0, z),
            ];
            [[0, 1, 2], [0, 3, 2]].map(|indices| Primitive {
                vertices: indices.map(|i| points[i]),
                surface: Surface::Solid(color),
                opacity,
            })
        };
        // Deliberately submit near glass before the cockpit and far glass.
        let mut primitives = Vec::from(quad(3.0, Color::new(0, 0, 200), 0.5));
        for _ in 0..70 {
            primitives.extend(quad(1.0, Color::new(0, 0, 0), 1.0));
        }
        primitives.extend(quad(2.0, Color::new(200, 0, 0), 0.5));
        primitives.extend(quad(4.0, Color::new(0, 255, 0), 0.0));
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut fb = Framebuffer::new(8, 8);
            fb.set_clip(1, 1, 6, 6);
            fb.point(2, 3, 2.5, 0x00c800, true);
            fb.point(3, 2, 5.0, 0xabcdef, true);
            pool.install(|| draw(&mut fb, &primitives));
            // Includes both windings and the shared diagonal.
            for (x, y) in [(1, 1), (4, 4), (5, 2), (2, 5)] {
                assert_eq!(fb.pixels[y * 8 + x], 0x320064);
                assert_eq!(fb.depth[y * 8 + x], 1.0);
            }
            assert_eq!(fb.pixels[3 * 8 + 2], 0x006464);
            assert_eq!(fb.depth[3 * 8 + 2], 2.5);
            assert_eq!(fb.pixels[2 * 8 + 3], 0xabcdef);
            assert_eq!(fb.pixels[0], crate::framebuffer::BACKGROUND);
        }
    }

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
                opacity: 1.0,
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
