use crate::renderer::WorldTransform;
use nalgebra_glm::Vec3;

pub struct Galaxy {
    pub name: &'static str,
    pub subtitle: &'static str,
    pub description: [&'static str; 3],
    pub planets: &'static [usize],
    pub accent: u32,
    pub sky: u32,
    pub map: (f32, f32),
    pub seed: u32,
    pub debris: usize,
}

pub const GALAXIES: [Galaxy; 3] = [
    Galaxy {
        name: "Aurora",
        subtitle: "FRONTERA DE EXPLORACION",
        description: [
            "Cuatro mundos, cuatro horizontes.",
            "Tormentas de arena, tuneles de roca",
            "y gigantes de nubes por descubrir.",
        ],
        planets: &[0, 1, 2, 3],
        accent: 0x78eed5,
        sky: 0x0d1725,
        map: (-285.0, -35.0),
        seed: 13875,
        debris: 0,
    },
    Galaxy {
        name: "Helice",
        subtitle: "CORREDOR DE POLVO ESTELAR",
        description: [
            "Un archipielago entre nubes doradas.",
            "Siroco y Jovara marcan el camino",
            "a traves de un cinturon de asteroides.",
        ],
        planets: &[0, 2],
        accent: 0xf1b671,
        sky: 0x26170f,
        map: (295.0, -145.0),
        seed: 71029,
        debris: 42,
    },
    Galaxy {
        name: "Umbria",
        subtitle: "REGION DE HIELO Y PIEDRA",
        description: [
            "Un paisaje frio, silencioso y remoto.",
            "Boreal ilumina los pasos de Oquedad",
            "entre fragmentos de roca azulada.",
        ],
        planets: &[1, 3],
        accent: 0xa9afff,
        sky: 0x17142c,
        map: (30.0, 180.0),
        seed: 94013,
        debris: 64,
    },
];

pub fn planet_transform(galaxy: usize, planet: usize) -> WorldTransform {
    let original = [
        ([-36.0, 7.0, -105.0], 22.0, 0.15),
        ([42.0, 3.0, -135.0], 25.0, -0.45),
        ([0.0, 57.0, -215.0], 35.0, 0.0),
        ([-145.0, -85.0, -235.0], 42.0, 0.35),
    ];
    let (position, radius, yaw) = match (galaxy, planet) {
        (1, 0) => ([-48.0, -12.0, -150.0], 30.0, 0.8),
        (1, 2) => ([65.0, 40.0, -235.0], 46.0, 0.2),
        (2, 1) => ([-42.0, 8.0, -145.0], 32.0, -0.7),
        (2, 3) => ([78.0, 20.0, -235.0], 49.0, 0.9),
        _ => original[planet],
    };
    WorldTransform {
        position: Vec3::from(position),
        radius,
        yaw,
    }
}

#[derive(Clone, Copy)]
pub struct GalaxyView {
    pub selected: usize,
    pub seconds: f32,
    pub zoom: f32,
    pub angle: f32,
    pub pan: (f32, f32),
    pub hover: Option<usize>,
    pub hover_button: bool,
    pub hover_back: bool,
    pub details_open: bool,
}

impl Default for GalaxyView {
    fn default() -> Self {
        Self {
            selected: 0,
            seconds: 0.0,
            zoom: 1.0,
            angle: 0.0,
            pan: (0.0, 0.0),
            hover: None,
            hover_button: false,
            hover_back: false,
            details_open: false,
        }
    }
}

impl GalaxyView {
    pub fn point(&self, index: usize) -> (f32, f32) {
        let (x, y) = GALAXIES[index].map;
        let (sin, cos) = (self.angle + self.seconds * 0.018).sin_cos();
        (
            640.0 + (x * cos - y * sin) * self.zoom + self.pan.0,
            420.0 + (x * sin + y * cos) * self.zoom + self.pan.1,
        )
    }

    pub fn hit(&self, point: (f32, f32)) -> Option<usize> {
        if !(0.0..1280.0).contains(&point.0)
            || !(120.0..720.0).contains(&point.1)
            || (self.details_open && crate::menus::DESTINATION_PANEL.contains(point))
        {
            return None;
        }
        (0..GALAXIES.len())
            .filter_map(|index| {
                let center = self.point(index);
                let distance = (center.0 - point.0).powi(2) + (center.1 - point.1).powi(2);
                (distance < (115.0 * self.zoom).powi(2)).then_some((index, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|entry| entry.0)
    }

    pub fn select(&mut self, index: usize) {
        self.selected = index;
        self.details_open = true;
    }

    pub fn advance(&mut self, dt: f32) {
        if dt.is_finite() && dt > 0.0 {
            self.seconds += dt.min(0.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptions_require_selection_and_automatic_motion_preserves_picking() {
        let mut view = GalaxyView::default();
        assert!(!view.details_open);
        let original = view.point(0);
        for _ in 0..300 {
            view.advance(1.0 / 60.0);
        }
        assert!(!view.details_open);
        assert!((view.point(0).1 - original.1).abs() > 10.0);
        for index in 0..GALAXIES.len() {
            assert_eq!(view.hit(view.point(index)), Some(index));
        }
        view.select(2);
        assert!(view.details_open);
        assert_eq!(view.selected, 2);
        assert_eq!(view.hit((950.0, 420.0)), None);
    }
    #[test]
    fn transformed_map_picks_the_visible_destination() {
        for zoom in [0.65, 1.0, 1.35] {
            let view = GalaxyView {
                zoom,
                angle: 0.3,
                pan: (20.0, -15.0),
                ..Default::default()
            };
            for index in 0..GALAXIES.len() {
                assert_eq!(view.hit(view.point(index)), Some(index));
            }
        }
        assert!(GalaxyView::default().hit((1100.0, 420.0)).is_none());
    }
}
