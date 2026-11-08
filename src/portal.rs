//! Two persistent, world-space portals and swept, momentum-preserving traversal.
use crate::collision::SHIP_RADIUS;
use nalgebra_glm::Vec3;

pub const RADIUS: f32 = 10.2;
pub const PLACE_DISTANCE: f32 = 32.0;
const EXIT_CLEARANCE: f32 = SHIP_RADIUS + 0.65;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Yellow = 0,
    Green = 1,
}

impl Kind {
    pub const ALL: [Self; 2] = [Self::Yellow, Self::Green];

    pub fn color(self) -> u32 {
        match self {
            Self::Yellow => 0xffdc34,
            Self::Green => 0x97ee55,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Yellow => "AMARILLO",
            Self::Green => "VERDE",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Portal {
    pub center: Vec3,
    pub normal: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

impl Portal {
    pub fn new(center: Vec3, normal: Vec3) -> Option<Self> {
        if !center.iter().chain(normal.iter()).all(|x| x.is_finite())
            || normal.norm_squared() < 1e-8
        {
            return None;
        }
        let normal = normal.normalize();
        let reference = if normal.y.abs() > 0.99 {
            Vec3::z()
        } else {
            Vec3::y()
        };
        let right = reference.cross(&normal).normalize();
        Some(Self {
            center,
            normal,
            right,
            up: normal.cross(&right),
        })
    }

    pub fn rotate_to(self, exit: Self, vector: Vec3) -> Vec3 {
        // A half-turn around local up makes incoming velocity point out of the exit.
        -exit.right * vector.dot(&self.right) + exit.up * vector.dot(&self.up)
            - exit.normal * vector.dot(&self.normal)
    }

    pub fn point_to(self, exit: Self, point: Vec3) -> Vec3 {
        exit.center + self.rotate_to(exit, point - self.center)
    }
}

pub struct Transit {
    pub entry_point: Vec3,
    pub exit_start: Vec3,
    pub position: Vec3,
    entry: Portal,
    exit: Portal,
}

impl Transit {
    pub fn exit_center(&self) -> Vec3 {
        self.exit.center
    }

    pub fn rotate(&self, vector: Vec3) -> Vec3 {
        self.entry.rotate_to(self.exit, vector)
    }
}

#[derive(Default)]
pub struct Pair {
    enabled: bool,
    slots: [Option<Portal>; 2],
    cooldown: f32,
    pub flash: f32,
    pub notice: Option<&'static str>,
    notice_left: f32,
    pub traversals: u32,
}

impl Pair {
    pub fn set_enabled(&mut self, enabled: bool) {
        if !enabled {
            *self = Self::default();
        }
        self.enabled = enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn get(&self, kind: Kind) -> Option<Portal> {
        self.slots[kind as usize]
    }

    pub fn linked(&self) -> bool {
        self.slots.iter().all(Option::is_some)
    }

    pub fn connection(&self, kind: Kind) -> Option<(Portal, Portal)> {
        Some((self.get(kind)?, self.slots[1 - kind as usize]?))
    }

    pub fn notify(&mut self, text: &'static str) {
        self.notice = Some(text);
        self.notice_left = 2.5;
    }

    pub fn place(&mut self, kind: Kind, portal: Portal) -> bool {
        if !self.enabled {
            return false;
        }
        if self.slots[1 - kind as usize]
            .is_some_and(|other| (other.center - portal.center).norm() < RADIUS * 2.0 + 3.0)
        {
            self.notify("Separa los portales: apunta hacia otro lugar.");
            return false;
        }
        self.slots[kind as usize] = Some(portal);
        self.notify(if self.linked() {
            "Portales enlazados. Atraviesa cualquiera de los dos."
        } else {
            "Coloca el otro portal para enlazarlos."
        });
        true
    }

    pub fn tick(&mut self, dt: f32) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.flash = (self.flash - dt).max(0.0);
        self.notice_left = (self.notice_left - dt).max(0.0);
        if self.notice_left == 0.0 {
            self.notice = None;
        }
    }

    pub fn crossing(&self, start: Vec3, end: Vec3) -> Option<Transit> {
        if !self.enabled || self.cooldown > 0.0 {
            return None;
        }
        let [Some(a), Some(b)] = self.slots else {
            return None;
        };
        let mut nearest: Option<(f32, Transit)> = None;
        for (entry, exit) in [(a, b), (b, a)] {
            let d0 = (start - entry.center).dot(&entry.normal);
            let d1 = (end - entry.center).dot(&entry.normal);
            if d0 * d1 > 0.0 || (d0 - d1).abs() < 1e-7 {
                continue;
            }
            let t = d0 / (d0 - d1);
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let entry_point = start + (end - start) * t;
            let offset = entry_point - entry.center;
            let x = offset.dot(&entry.right);
            let y = offset.dot(&entry.up);
            // The whole ship must fit, including at boost speed and low frame rates.
            if x * x + y * y > (RADIUS - SHIP_RADIUS).powi(2) {
                continue;
            }
            let side = if d0 >= 0.0 { 1.0 } else { -1.0 };
            let exit_start =
                exit.center - exit.right * x + exit.up * y + exit.normal * (side * EXIT_CLEARANCE);
            let transit = Transit {
                entry_point,
                exit_start,
                position: exit_start + entry.rotate_to(exit, end - entry_point),
                entry,
                exit,
            };
            if nearest.as_ref().is_none_or(|(old, _)| t < *old) {
                nearest = Some((t, transit));
            }
        }
        nearest.map(|(_, transit)| transit)
    }

    pub fn complete_transit(&mut self) {
        self.cooldown = 0.4;
        self.flash = 0.45;
        self.traversals += 1;
        self.notify("Salto de portal completado.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> Pair {
        let mut pair = Pair::default();
        pair.set_enabled(true);
        pair.place(Kind::Yellow, Portal::new(Vec3::zeros(), Vec3::z()).unwrap());
        pair.place(
            Kind::Green,
            Portal::new(Vec3::new(100.0, 20.0, 0.0), Vec3::x()).unwrap(),
        );
        pair
    }

    #[test]
    fn swept_crossing_rotates_momentum_keeps_offset_and_remaining_distance() {
        let pair = pair();
        let transit = pair
            .crossing(Vec3::new(2.0, 1.0, 50.0), Vec3::new(2.0, 1.0, -50.0))
            .unwrap();
        assert!((transit.position - Vec3::new(152.35, 21.0, 2.0)).norm() < 1e-4);
        let velocity = Vec3::new(4.0, 2.0, -155.0);
        assert!((transit.rotate(velocity).norm() - velocity.norm()).abs() < 1e-4);
        assert!((transit.rotate(-Vec3::z()) - Vec3::x()).norm() < 1e-5);
    }

    #[test]
    fn both_colors_and_both_faces_work_but_missing_partner_and_rim_misses_do_not() {
        let pair = pair();
        assert!(pair.crossing(Vec3::z() * -2.0, Vec3::z() * 2.0).is_some());
        assert!(
            pair.crossing(Vec3::new(102.0, 20.0, 0.0), Vec3::new(98.0, 20.0, 0.0))
                .is_some()
        );
        let edge = RADIUS - SHIP_RADIUS;
        assert!(
            pair.crossing(
                Vec3::new(edge - 0.1, 0.0, 2.0),
                Vec3::new(edge - 0.1, 0.0, -2.0)
            )
            .is_some()
        );
        assert!(
            pair.crossing(
                Vec3::new(edge + 0.1, 0.0, 2.0),
                Vec3::new(edge + 0.1, 0.0, -2.0)
            )
            .is_none()
        );
        let mut single = Pair::default();
        single.set_enabled(true);
        single.place(Kind::Yellow, pair.get(Kind::Yellow).unwrap());
        assert!(single.crossing(Vec3::z(), -Vec3::z()).is_none());
        assert!(pair.crossing(Vec3::zeros(), Vec3::zeros()).is_none());
    }

    #[test]
    fn replacing_one_color_preserves_partner_and_overlap_rejection_keeps_old_portal() {
        let mut pair = pair();
        let partner = pair.get(Kind::Green).unwrap();
        let replacement = Portal::new(Vec3::new(-100.0, 0.0, 0.0), Vec3::z()).unwrap();
        assert!(pair.place(Kind::Yellow, replacement));
        assert_eq!(pair.get(Kind::Green).unwrap().center, partner.center);
        assert!(!pair.place(Kind::Yellow, partner));
        assert_eq!(pair.get(Kind::Yellow).unwrap().center, replacement.center);
        pair.set_enabled(false);
        assert!(!pair.linked());
        assert!(!pair.place(Kind::Yellow, replacement));
    }

    #[test]
    fn cooldown_prevents_feedback_and_invalid_frames_are_rejected() {
        let mut pair = pair();
        pair.complete_transit();
        assert!(pair.crossing(Vec3::z(), -Vec3::z()).is_none());
        pair.tick(0.5);
        assert!(pair.crossing(Vec3::z(), -Vec3::z()).is_some());
        assert!(Portal::new(Vec3::zeros(), Vec3::zeros()).is_none());
        assert!(Portal::new(Vec3::repeat(f32::NAN), Vec3::z()).is_none());
        let vertical = Portal::new(Vec3::zeros(), Vec3::y()).unwrap();
        assert!((vertical.right.cross(&vertical.up) - vertical.normal).norm() < 1e-5);
    }
}
