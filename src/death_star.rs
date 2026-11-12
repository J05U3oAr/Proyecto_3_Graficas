//! TIE-only orbital strike. The target is locked once; orbit motion stays live.
use crate::{
    flight::Flight,
    framebuffer::Framebuffer,
    local_navigation::Reference,
    model::Model,
    renderer::{self, FlightView, WorldTransform},
};
use nalgebra_glm::Vec3;
use std::{f32::consts::TAU, sync::OnceLock};

pub const FIRE: f32 = 4.8;
pub const IMPACT: f32 = 5.5;
pub const DURATION: f32 = 11.0;
pub const COOLDOWN: f32 = 22.0;

pub fn model() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| {
        Model::load(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets/death-star/death-star.obj"),
        )
        .expect("Modelo de la Estrella de la Muerte")
    })
}

pub struct Strike {
    pub target: Reference,
    pub age: f32,
    side: Vec3,
    pub camera_side: f32,
    impact_reported: bool,
}

impl Strike {
    pub fn new(target: Reference, flight: &Flight) -> Self {
        let right = flight.heading.basis().0;
        let side = Vec3::new(right.x, 0.0, right.z)
            .try_normalize(1e-5)
            .unwrap_or(Vec3::x());
        Self {
            target,
            age: 0.0,
            side,
            camera_side: 1.0,
            impact_reported: false,
        }
    }
    pub fn take_impact(&mut self) -> Option<&'static str> {
        if self.age >= IMPACT && !self.impact_reported {
            self.impact_reported = true;
            Some(self.target.name)
        } else {
            None
        }
    }
    pub fn stage(&self) -> &'static str {
        if self.age < 1.8 {
            "ESTRELLA DE LA MUERTE EN CAMINO"
        } else if self.age < FIRE {
            "CARGANDO SUPERLASER"
        } else if self.age < IMPACT {
            "DISPARANDO SUPERLASER"
        } else if self.age < 9.0 {
            "PLANETA DESTRUIDO"
        } else {
            "RETIRANDO ESTACION"
        }
    }
    pub fn center(&self, seconds: f32) -> Vec3 {
        self.target.center_at(seconds)
    }
    pub fn radius(&self) -> f32 {
        self.target.radius * 0.9
    }
    pub fn rotate_deployment(&mut self, angle: f32) {
        self.side = crate::local_navigation::rotate(self.side, Vec3::y(), angle);
    }
    fn direction(&self) -> Vec3 {
        self.side * (1.0_f32 - 0.42 * 0.42).sqrt() + Vec3::y() * 0.42
    }
    pub fn station(&self, seconds: f32) -> Vec3 {
        let arrival = 1.0 - crate::noise::smoothstep(0.0, 1.8, self.age);
        let departure = crate::noise::smoothstep(9.2, DURATION, self.age);
        self.firing_station(seconds)
            - self.side * (arrival * arrival + departure * departure) * self.target.radius * 6.0
    }
    pub fn firing_station(&self, seconds: f32) -> Vec3 {
        self.center(seconds) - self.direction() * self.target.radius * 4.4
    }
    pub fn view(&self, flight: &Flight, width: usize, height: usize) -> FlightView {
        let radius = self.target.radius;
        let center = self.center(flight.seconds);
        let station = center - self.direction() * radius * 4.4;
        let midpoint = (station + center) * 0.5;
        let perpendicular = self.side.cross(&Vec3::y());
        // A three-quarter view keeps the dish, converging beams and planet in frame.
        let eye = midpoint
            + (perpendicular * (4.8 * self.camera_side) + self.side * 3.6 + Vec3::y() * 2.0)
                * radius;
        FlightView::new(flight, width, height).looking_at(eye, midpoint)
    }
    pub fn draw(&self, fb: &mut Framebuffer, view: &FlightView, seconds: f32) {
        let center = self.station(seconds);
        let r = self.radius();
        let yaw = self.side.x.atan2(self.side.z);
        renderer::draw_world_model_shaded(
            fb,
            model(),
            view,
            WorldTransform {
                position: center,
                radius: r,
                yaw,
            },
            Some(center + axis_light(self.side, self.camera_side) * r * 15.0),
            crate::shaders::SurfaceEffect::None,
        );
        let axis = self.direction();
        let dish_up = Vec3::y() * (1.0_f32 - 0.42 * 0.42).sqrt() - self.side * 0.42;
        let dish_right = dish_up.cross(&axis).normalize();
        let focus = center + axis * r * 1.6;
        if (2.0..IMPACT + 0.12).contains(&self.age) {
            let charge = crate::noise::smoothstep(2.0, FIRE, self.age);
            for i in 0..8 {
                let angle = TAU * i as f32 / 8.0;
                let emitter = center
                    + axis * r * 0.93
                    + (dish_right * angle.cos() + dish_up * angle.sin()) * r * 0.28;
                glow(fb, view, emitter, r * 0.028, 0xbaff81, charge);
                if self.age > 3.0 + i as f32 * 0.12 {
                    beam(fb, view, emitter, focus, r * 0.008, charge);
                }
            }
            glow(fb, view, focus, r * 0.10, 0xc8ff91, charge * 0.9);
            if self.age >= FIRE {
                let target = self.center(seconds);
                let end = target - axis * self.target.radius;
                let progress = ((self.age - FIRE) / (IMPACT - FIRE)).clamp(0.0, 1.0);
                beam(
                    fb,
                    view,
                    focus,
                    focus + (end - focus) * progress,
                    r * 0.024,
                    1.0,
                );
                glow(fb, view, focus, r * 0.15, 0xe0ffc0, 1.0);
            }
        }
        if self.age >= IMPACT {
            self.explosion(fb, view, seconds);
        }
    }
    fn explosion(&self, fb: &mut Framebuffer, view: &FlightView, seconds: f32) {
        let age = self.age - IMPACT;
        let center = self.center(seconds);
        let r = self.target.radius;
        let fade = (1.0 - age / (DURATION - IMPACT)).max(0.0);
        // Solid rotating fragments, with molten edges, leave a traversable void.
        for i in 0..180 {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / 180.0;
            let angle = i as f32 * 2.399_963;
            let direction = Vec3::new(
                angle.cos() * (1.0 - y * y).sqrt(),
                y,
                angle.sin() * (1.0 - y * y).sqrt(),
            );
            let speed = 0.25 + random(i, 1.0) * 0.60;
            let p = center + direction * r * (0.65 + age * speed);
            let size = r * (0.035 + random(i, 2.0) * 0.07) * fade;
            let spin = angle + age * (random(i, 3.0) * 3.0 + 0.4);
            let (right, up, forward) = view.basis();
            let a = (right * spin.cos() + up * spin.sin()) * size;
            let b = (-right * spin.sin() + up * spin.cos()) * size * 0.7;
            let corners = [p + a, p - a * 0.5 + b, p - a * 0.5 - b, p + forward * size];
            for (face, indices) in [[0, 1, 2], [0, 3, 1], [1, 3, 2], [2, 3, 0]]
                .iter()
                .enumerate()
            {
                let [a, b, c] = indices.map(|j| corners[j]);
                let (Some(a), Some(b), Some(c)) =
                    (view.project(a), view.project(b), view.project(c))
                else {
                    continue;
                };
                let molten = crate::ui::mix(
                    0x393d47,
                    0xffa34c,
                    (1.0 - age / 3.0).max(0.0) * random(i, 4.0),
                );
                let tint = crate::ui::mix(0x131922, molten, 0.45 + face as f32 * 0.17);
                crate::triangle::fill(
                    fb,
                    &a,
                    &b,
                    &c,
                    crate::color::Color::new((tint >> 16) as u8, (tint >> 8) as u8, tint as u8),
                );
            }
            if i % 3 == 0 {
                glow(
                    fb,
                    view,
                    p,
                    size * 2.8,
                    0xff7b26,
                    fade * (1.0 - age / 4.0).max(0.0) * 0.5,
                );
            }
        }
        // Expanding equatorial shock wave, drawn as world-space segments.
        let (right, _, forward) = view.basis();
        let ring = r * (1.05 + age * 1.7);
        for i in 0..160 {
            let point = |a: f32| center + (right * a.cos() + forward * a.sin()) * ring;
            beam_color(
                fb,
                view,
                point(TAU * i as f32 / 160.0),
                point(TAU * (i + 1) as f32 / 160.0),
                r * 0.012,
                0xffbe77,
                fade * 0.65,
            );
        }
        glow(
            fb,
            view,
            center,
            r * (1.0 + age * 0.5),
            0xff9c44,
            (1.0 - age / 2.3).max(0.0) * 0.85,
        );
        glow(
            fb,
            view,
            center,
            r * 1.2,
            0xfff3ca,
            (1.0 - age / 0.55).max(0.0),
        );
    }
}

fn axis_light(side: Vec3, camera_side: f32) -> Vec3 {
    side * 0.8 + Vec3::y() * 1.5 + side.cross(&Vec3::y()) * (2.0 * camera_side)
}

fn random(i: usize, salt: f32) -> f32 {
    ((i as f32 * 127.1 + salt * 311.7).sin() * 43_758.547)
        .fract()
        .abs()
}

fn glow(
    fb: &mut Framebuffer,
    view: &FlightView,
    world: Vec3,
    radius: f32,
    color: u32,
    strength: f32,
) {
    if strength <= 0.0 {
        return;
    }
    let Some(p) = view.project(world) else {
        return;
    };
    let radius = (radius * renderer::FLIGHT_FOCAL * p.z).max(1.0);
    let bounds = (
        (p.x - radius).max(0.0) as usize,
        (p.y - radius).max(0.0) as usize,
        (p.x + radius).max(0.0).ceil() as usize,
        (p.y + radius).max(0.0).ceil() as usize,
    );
    fb.shade_rect(bounds, |x, y, pixel, depth| {
        let d = ((x as f32 + 0.5 - p.x).powi(2) + (y as f32 + 0.5 - p.y).powi(2)) / radius.powi(2);
        if d < 1.0 && p.z >= *depth {
            *pixel = crate::framebuffer::emission_color(
                *pixel,
                color,
                (1.0 - d).powi(2) * strength,
                0.0,
            );
        }
    });
}
fn beam(fb: &mut Framebuffer, view: &FlightView, a: Vec3, b: Vec3, r: f32, strength: f32) {
    beam_color(fb, view, a, b, r, 0x72ff46, strength);
}
fn beam_color(
    fb: &mut Framebuffer,
    view: &FlightView,
    a: Vec3,
    b: Vec3,
    r: f32,
    color: u32,
    strength: f32,
) {
    let (Some(a), Some(b)) = (view.project(a), view.project(b)) else {
        return;
    };
    let width = (r * renderer::FLIGHT_FOCAL * a.z.max(b.z)).max(1.0);
    let reach = width * 4.0;
    let bounds = (
        (a.x.min(b.x) - reach).max(0.0) as usize,
        (a.y.min(b.y) - reach).max(0.0) as usize,
        (a.x.max(b.x) + reach).max(0.0).ceil() as usize,
        (a.y.max(b.y) + reach).max(0.0).ceil() as usize,
    );
    let delta = nalgebra_glm::Vec2::new(b.x - a.x, b.y - a.y);
    fb.shade_rect(bounds, |x, y, pixel, depth| {
        let point = nalgebra_glm::Vec2::new(x as f32 + 0.5 - a.x, y as f32 + 0.5 - a.y);
        let t = (point.dot(&delta) / delta.norm_squared().max(0.001)).clamp(0.0, 1.0);
        let distance = (point - delta * t).norm() / width;
        if a.z + (b.z - a.z) * t > *depth {
            let glow = (-distance * distance * 0.3).exp() * strength;
            *pixel = crate::framebuffer::emission_color(*pixel, color, glow, 0.0);
            *pixel = crate::framebuffer::emission_color(
                *pixel,
                0xf2ffdc,
                (-distance * distance * 3.0).exp() * strength,
                0.0,
            );
        }
    });
}
