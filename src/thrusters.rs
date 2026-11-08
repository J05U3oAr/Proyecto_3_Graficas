use crate::{
    fleet::Ship,
    flight::Flight,
    framebuffer::Framebuffer,
    renderer::{FLIGHT_FOCAL, FlightView},
};

pub fn draw(fb: &mut Framebuffer, ship: &Ship, flight: &Flight, seconds: f32) {
    let speed = flight.velocity.norm();
    if ship.lights.exhausts.is_empty()
        || speed < 0.15
        || flight.crash.is_some()
        || flight.is_captured()
    {
        return;
    }
    let power = (speed / crate::flight::CRUISE_SPEED).clamp(0.0, 3.0);
    let fade = (speed / 4.0).min(1.0);
    let length = 0.55 + power * 0.55;
    let (right, up, forward) = flight.ship_basis();
    let view = FlightView::new(flight, fb.width, fb.height);
    for (engine, local) in ship.lights.exhausts.iter().enumerate() {
        let nozzle = flight.position
            + (-right * local.x + up * local.y + forward * local.z) * crate::collision::SHIP_RADIUS;
        // Los segmentos siguen el eje del motor y respetan la profundidad del casco y los planetas.
        for segment in (0..32).rev() {
            let t = segment as f32 / 32.0;
            let phase = seconds * 32.0 - t * 19.0 + engine as f32 * 2.7;
            let flutter = phase.sin() * 0.6 + (phase * 1.7).sin() * 0.4;
            let position = nozzle - forward * (t * length)
                + (right * phase.sin() + up * (phase * 1.3).cos()) * (t * t * 0.025);
            let radius = (0.07 + power * 0.012) * (1.0 - t).powf(0.65) * (1.0 + flutter * 0.15);
            let Some(center) = view.project(position) else {
                continue;
            };
            let radius = (radius * FLIGHT_FOCAL * center.z).max(0.6);
            let bounds = (
                (center.x - radius * 2.0).max(0.0) as usize,
                (center.y - radius * 2.0).max(0.0) as usize,
                (center.x + radius * 2.0).ceil().max(0.0) as usize,
                (center.y + radius * 2.0).ceil().max(0.0) as usize,
            );
            fb.shade_rect(bounds, |x, y, pixel, depth| {
                if center.z <= *depth {
                    return;
                }
                let distance = ((x as f32 + 0.5 - center.x).powi(2)
                    + (y as f32 + 0.5 - center.y).powi(2))
                    / radius.powi(2);
                let glow = (-distance * 1.5).exp() * (1.0 - t) * fade * 0.32;
                let color = if distance < 0.35 && t < 0.45 {
                    0xeaf6ff
                } else if t < 0.65 {
                    0xffad38
                } else {
                    0xed4918
                };
                *pixel = crate::framebuffer::emission_color(*pixel, color, glow, 0.0);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::Vec3;
    #[test]
    fn exhaust_animates_only_on_silencer_in_motion_and_respects_occlusion() {
        let ships = crate::fleet::load().unwrap();
        assert_eq!(ships[1].name, "TIE Silencer");
        let mut flight = Flight::new();
        let mut frame = Framebuffer::new(1280, 840);
        let empty = frame.pixels.clone();
        draw(&mut frame, &ships[1], &flight, 0.0);
        assert_eq!(frame.pixels, empty);
        flight.velocity = -Vec3::z() * 36.0;
        draw(&mut frame, &ships[0], &flight, 0.0);
        assert_eq!(frame.pixels, empty);
        draw(&mut frame, &ships[1], &flight, 0.0);
        assert!(
            frame
                .pixels
                .iter()
                .zip(&empty)
                .filter(|(a, b)| a != b)
                .count()
                > 50
        );
        let before = frame.pixels.clone();
        frame.clear();
        draw(&mut frame, &ships[1], &flight, 0.1);
        assert_ne!(before, frame.pixels);
        assert!(frame.depth.iter().all(|&d| d == f32::NEG_INFINITY));
        frame.depth.fill(f32::INFINITY);
        frame.pixels.fill(0x123456);
        draw(&mut frame, &ships[1], &flight, 0.2);
        assert!(frame.pixels.iter().all(|&p| p == 0x123456));
    }
}
