use crate::color::Color;
use crate::{framebuffer::Framebuffer, texture::Texture};
use nalgebra_glm::{Vec2, Vec3};

#[derive(Clone, Copy)]
pub struct TextureVertex {
    pub screen: Vec3,
    pub uv: Vec2,
    pub inverse_w: f32,
}

fn edge(a: &Vec3, b: &Vec3, x: f32, y: f32) -> f32 {
    (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
}

pub fn fill(framebuffer: &mut Framebuffer, a: &Vec3, b: &Vec3, c: &Vec3, color: Color) {
    rasterize(framebuffer, [*a, *b, *c], |_, _, _| color);
}

pub fn fill_textured(
    framebuffer: &mut Framebuffer,
    vertices: [TextureVertex; 3],
    texture: &Texture,
    tint: Color,
    intensity: f32,
) {
    let [a, b, c] = vertices;
    rasterize(framebuffer, vertices.map(|v| v.screen), |w0, w1, w2| {
        let inverse_w = a.inverse_w * w0 + b.inverse_w * w1 + c.inverse_w * w2;
        let uv =
            (a.uv * (w0 * a.inverse_w) + b.uv * (w1 * b.inverse_w) + c.uv * (w2 * c.inverse_w))
                / inverse_w;
        let color = texture.sample(uv);
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
    });
}

fn rasterize(
    framebuffer: &mut Framebuffer,
    vertices: [Vec3; 3],
    shade: impl Fn(f32, f32, f32) -> Color,
) {
    let [a, b, c] = &vertices;
    let area = edge(a, b, c.x, c.y);
    if area.abs() < 0.00001 {
        return;
    }
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as i32;
    let max_x =
        a.x.max(b.x)
            .max(c.x)
            .ceil()
            .min(framebuffer.width as f32 - 1.0) as i32;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
    let max_y =
        a.y.max(b.y)
            .max(c.y)
            .ceil()
            .min(framebuffer.height as f32 - 1.0) as i32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let w0 = edge(b, c, px, py) / area;
            let w1 = edge(c, a, px, py) / area;
            let w2 = edge(a, b, px, py) / area;
            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                let depth = w0 * a.z + w1 * b.z + w2 * c.z;
                framebuffer.point(x, y, depth, shade(w0, w1, w2).to_hex(), true);
            }
        }
    }
}

pub fn line(framebuffer: &mut Framebuffer, from: &Vec3, to: &Vec3, color: u32, depth_test: bool) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let steps = dx.abs().max(dy.abs()).ceil().max(1.0) as usize;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = (from.x + dx * t).round() as i32;
        let y = (from.y + dy * t).round() as i32;
        let z = from.z + (to.z - from.z) * t;
        framebuffer.point(x, y, z, color, depth_test);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_interpolation_uses_reciprocal_depth_and_respects_opaque_geometry() {
        let texture = Texture::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/uv-checker.png"
        ))
        .unwrap();
        let vertices = [
            TextureVertex {
                screen: Vec3::new(0.0, 0.0, 1.0),
                uv: Vec2::new(0.0, 0.0),
                inverse_w: 1.0,
            },
            TextureVertex {
                screen: Vec3::new(8.0, 0.0, 0.25),
                uv: Vec2::new(1.0, 0.0),
                inverse_w: 0.25,
            },
            TextureVertex {
                screen: Vec3::new(0.0, 8.0, 0.25),
                uv: Vec2::new(0.0, 1.0),
                inverse_w: 0.25,
            },
        ];
        let mut frame = Framebuffer::new(8, 8);
        fill_textured(
            &mut frame,
            vertices,
            &texture,
            Color::new(255, 255, 255),
            1.0,
        );
        assert_eq!(frame.pixels[2 * 8 + 2], Color::new(64, 38, 218).to_hex());
        frame.point(2, 2, 2.0, 0x123456, true);
        fill_textured(
            &mut frame,
            vertices,
            &texture,
            Color::new(255, 255, 255),
            1.0,
        );
        assert_eq!(frame.pixels[2 * 8 + 2], 0x123456);
    }
}
