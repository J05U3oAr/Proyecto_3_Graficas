use crate::color::Color;
use crate::framebuffer::Framebuffer;
use nalgebra_glm::Vec3;

fn edge(a: &Vec3, b: &Vec3, x: f32, y: f32) -> f32 {
    (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
}

pub fn fill(framebuffer: &mut Framebuffer, a: &Vec3, b: &Vec3, c: &Vec3, color: Color) {
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
                framebuffer.point(x, y, depth, color.to_hex(), true);
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
