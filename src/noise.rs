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
