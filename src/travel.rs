use crate::{
    framebuffer::Framebuffer,
    galaxy::GALAXIES,
    noise,
    ui::{blend_pixel, glow, mix, rule},
};
use rayon::prelude::*;
use std::f32::consts::TAU;

pub const DURATION: f32 = 2.8;

pub struct Travel {
    pub destination: usize,
    seconds: f32,
}

impl Travel {
    pub fn new(destination: usize) -> Self {
        Self {
            destination,
            seconds: 0.0,
        }
    }

    pub fn at(destination: usize, seconds: f32) -> Self {
        Self {
            destination,
            seconds: seconds.clamp(0.0, DURATION),
        }
    }

    pub fn advance(&mut self, dt: f32) -> bool {
        if dt.is_finite() && dt > 0.0 {
            self.seconds = (self.seconds + dt.min(0.1)).min(DURATION);
        }
        self.seconds >= DURATION
    }

    pub fn progress(&self) -> f32 {
        self.seconds / DURATION
    }
}

pub fn draw(fb: &mut Framebuffer, travel: &Travel, background: &[u32], arrival: &[u32]) {
    let progress = travel.progress();
    let width = fb.width;
    let height = fb.height;
    let center = (width as f32 * 0.5, height as f32 * 0.48);
    let scale = 1.0 + progress * progress * 2.5;
    fb.clear();
    fb.pixels
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, pixel) in row.iter_mut().enumerate() {
                let source_x = ((x as f32 - center.0) / scale + center.0)
                    .clamp(0.0, width as f32 - 1.0) as usize;
                let source_y = ((y as f32 - center.1) / scale + center.1)
                    .clamp(0.0, height as f32 - 1.0) as usize;
                *pixel = background[source_y * width + source_x];
            }
        });
    let accent = GALAXIES[travel.destination].accent;
    glow(fb, center, (260.0, 190.0), accent, progress * 0.16);
    let acceleration = noise::smoothstep(0.0, 0.55, progress);
    for i in 0..850 {
        let angle = noise::hash(i, 17, 43) * TAU;
        let z = (noise::hash(i, 29, 71) - travel.seconds * (0.15 + progress * 0.72))
            .rem_euclid(1.0)
            + 0.055;
        let spread = 40.0 + noise::hash(i, 101, 59) * 100.0;
        let front = (spread / z).min(1700.0);
        let back = spread / (z + 0.006 + acceleration * 0.15);
        let (sin, cos) = angle.sin_cos();
        let color = mix(accent, 0xffffff, 0.45 + acceleration * 0.45);
        let brightness = (0.25 + (1.0 - z).max(0.0) * 0.75).min(1.0);
        for segment in 0..5 {
            let t0 = segment as f32 / 5.0;
            let t1 = (segment + 1) as f32 / 5.0;
            let from = back + (front - back) * t0;
            let to = back + (front - back) * t1;
            rule(
                fb,
                (
                    (center.0 + cos * from) as i32,
                    (center.1 + sin * from * 0.72) as i32,
                ),
                (
                    (center.0 + cos * to) as i32,
                    (center.1 + sin * to * 0.72) as i32,
                ),
                mix(0x080611, color, brightness * (0.2 + t1 * 0.8)),
            );
        }
        if z > 0.5 {
            blend_pixel(
                fb,
                (center.0 + cos * front) as i32,
                (center.1 + sin * front * 0.72) as i32,
                0xffffff,
                0.7,
            );
        }
    }
    // La llegada se mezcla con el primer cuadro real del destino sin avanzar la fisica.
    let reveal = noise::smoothstep(0.78, 1.0, progress);
    let flash = (-(progress - 0.76).powi(2) / 0.0009).exp() * 0.5;
    fb.pixels
        .par_iter_mut()
        .zip(arrival.par_iter())
        .for_each(|(pixel, &target)| {
            *pixel = mix(mix(*pixel, 0xe9eaff, flash), target, reveal);
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn travel_finishes_by_elapsed_time_and_reveals_the_exact_destination() {
        let mut travel = Travel::new(1);
        assert!(!travel.advance(f32::NAN));
        for _ in 0..28 {
            travel.advance(0.1);
        }
        assert!(travel.advance(0.01));
        assert_eq!(travel.progress(), 1.0);
        let mut frame = Framebuffer::new(128, 84);
        let background = vec![0x101020; 128 * 84];
        let arrival = vec![0x804020; 128 * 84];
        draw(&mut frame, &travel, &background, &arrival);
        assert_eq!(frame.pixels, arrival);
        let mut halfway = Framebuffer::new(128, 84);
        draw(&mut halfway, &Travel::at(1, 1.4), &background, &arrival);
        assert_ne!(halfway.pixels, arrival);
        assert_ne!(halfway.pixels, background);
    }
}
