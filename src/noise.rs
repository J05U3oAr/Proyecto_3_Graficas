use nalgebra_glm::Vec3;

pub fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut value = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add((z as u32).wrapping_mul(2_147_483_647));
    value = (value ^ (value >> 13)).wrapping_mul(1_274_126_177);
    (value ^ (value >> 16)) as f32 / u32::MAX as f32
}

pub fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn noise(point: Vec3) -> f32 {
    let cell = point.map(|v| v.floor() as i32);
    let fraction = point - point.map(f32::floor);
    let fade = fraction.map(|t| t * t * (3.0 - 2.0 * t));
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let row = |y, z| {
        mix(
            hash(cell.x, cell.y + y, cell.z + z),
            hash(cell.x + 1, cell.y + y, cell.z + z),
            fade.x,
        )
    };
    mix(
        mix(row(0, 0), row(1, 0), fade.y),
        mix(row(0, 1), row(1, 1), fade.y),
        fade.z,
    )
}

/// Value noise and its analytic gradient share the same eight lattice samples.
pub fn noise_with_gradient(point: Vec3) -> (f32, Vec3) {
    let cell = point.map(|v| v.floor() as i32);
    let t = point - point.map(f32::floor);
    let fade = t.map(|v| v * v * (3.0 - 2.0 * v));
    let derivative = t.map(|v| 6.0 * v * (1.0 - v));
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c000 = hash(cell.x, cell.y, cell.z);
    let c100 = hash(cell.x + 1, cell.y, cell.z);
    let c010 = hash(cell.x, cell.y + 1, cell.z);
    let c110 = hash(cell.x + 1, cell.y + 1, cell.z);
    let c001 = hash(cell.x, cell.y, cell.z + 1);
    let c101 = hash(cell.x + 1, cell.y, cell.z + 1);
    let c011 = hash(cell.x, cell.y + 1, cell.z + 1);
    let c111 = hash(cell.x + 1, cell.y + 1, cell.z + 1);
    let x00 = mix(c000, c100, fade.x);
    let x10 = mix(c010, c110, fade.x);
    let x01 = mix(c001, c101, fade.x);
    let x11 = mix(c011, c111, fade.x);
    let y0 = mix(x00, x10, fade.y);
    let y1 = mix(x01, x11, fade.y);
    let dx = mix(
        mix(c100 - c000, c110 - c010, fade.y),
        mix(c101 - c001, c111 - c011, fade.y),
        fade.z,
    ) * derivative.x;
    let dy = mix(x10 - x00, x11 - x01, fade.z) * derivative.y;
    (
        mix(y0, y1, fade.z),
        Vec3::new(dx, dy, (y1 - y0) * derivative.z),
    )
}

pub fn fbm(point: Vec3) -> f32 {
    let (mut sum, mut weight, mut amplitude, mut frequency) = (0.0, 0.0, 0.5, 1.0);
    for _ in 0..5 {
        sum += noise(point * frequency) * amplitude;
        weight += amplitude;
        amplitude *= 0.5;
        frequency *= 2.03;
    }
    sum / weight
}

// Quintic gradient noise for continuous surface detail without square patches.
pub fn gradient_noise(p: Vec3) -> f32 {
    let cell = p.map(|v| v.floor() as i32);
    let f = p - p.map(f32::floor);
    let fade = f.map(|t| t * t * t * (t * (t * 6.0 - 15.0) + 10.0));
    let corner = |x: i32, y: i32, z: i32| {
        let h = (hash(cell.x + x, cell.y + y, cell.z + z) * 255.0) as u32 & 15;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_gradient_matches_finite_differences_including_negative_coordinates() {
        for p in [
            Vec3::new(0.23, 1.37, -2.41),
            Vec3::new(-4.2, -0.7, 3.17),
            Vec3::new(1.0, 2.0, 3.0),
        ] {
            let (value, gradient) = noise_with_gradient(p);
            assert!((value - noise(p)).abs() < 0.00001);
            for axis in 0..3 {
                let mut offset = Vec3::zeros();
                offset[axis] = 0.001;
                let measured = (noise(p + offset) - noise(p - offset)) / 0.002;
                assert!((gradient[axis] - measured).abs() < 0.003);
            }
        }
    }
}
