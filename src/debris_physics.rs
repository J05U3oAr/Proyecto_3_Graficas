//! Deterministic free-fall trajectories shared by rendering and mesh collisions.
//! Sampling a preview never advances the game clock; earlier times can be replayed.
use crate::black_hole::BlackHole;
use nalgebra_glm::{DVec3, Vec3};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};

const RATE: f64 = 120.0;
const STEP: f64 = 1.0 / RATE;
const CHECKPOINT_TICKS: u64 = 240;
const MAX_CHECKPOINTS: usize = 128;
const SAMPLE_CACHE: usize = 8;

#[derive(Clone, Copy)]
pub struct Body {
    pub position: Vec3,
    pub velocity: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    pub position: Vec3,
    pub velocity: Vec3,
    pub captured: bool,
}

// Double precision preserves the small displacement of far-away, slowly falling rocks.
#[derive(Clone, Copy)]
struct Particle {
    position: DVec3,
    velocity: DVec3,
    captured: bool,
}

impl Particle {
    fn state(self) -> State {
        State {
            position: self.position.cast(),
            velocity: self.velocity.cast(),
            captured: self.captured,
        }
    }
}

struct Cache {
    tick: u64,
    states: Vec<Particle>,
    checkpoints: BTreeMap<u64, Arc<[Particle]>>,
    recent: VecDeque<(u64, Arc<[Particle]>)>,
    samples: VecDeque<(u32, Arc<[State]>)>,
}

pub struct Field {
    hole: BlackHole,
    bodies: Vec<Body>,
    cache: Mutex<Cache>,
}

impl Field {
    pub fn new(hole: BlackHole, bodies: Vec<Body>) -> Self {
        let states: Vec<_> = bodies
            .iter()
            .map(|body| Particle {
                position: body.position.cast(),
                velocity: body.velocity.cast(),
                captured: hole.captures_sphere(body.position, body.position, body.radius),
            })
            .collect();
        let mut checkpoints = BTreeMap::new();
        checkpoints.insert(0, Arc::from(states.clone()));
        Self {
            hole,
            bodies,
            cache: Mutex::new(Cache {
                tick: 0,
                states,
                checkpoints,
                recent: VecDeque::new(),
                samples: VecDeque::new(),
            }),
        }
    }

    /// A fixed 120 Hz velocity-Verlet step gives the same trajectory at 10 or 60 FPS.
    fn step(&self, states: &mut [Particle]) {
        for (state, body) in states.iter_mut().zip(&self.bodies) {
            if state.captured {
                continue;
            }
            let first: DVec3 = self
                .hole
                .free_fall_acceleration(state.position.cast())
                .cast();
            let next = state.position + state.velocity * STEP + first * (0.5 * STEP * STEP);
            if self
                .hole
                .captures_sphere(state.position.cast(), next.cast(), body.radius)
            {
                state.position = next;
                state.velocity = DVec3::zeros();
                state.captured = true;
            } else {
                let second: DVec3 = self.hole.free_fall_acceleration(next.cast()).cast();
                state.position = next;
                state.velocity += (first + second) * (0.5 * STEP);
            }
        }
    }

    fn seek(&self, cache: &mut Cache, tick: u64) {
        if tick < cache.tick {
            let (&checkpoint, states) = cache.checkpoints.range(..=tick).next_back().unwrap();
            if let Some((recent_tick, recent_states)) = cache
                .recent
                .iter()
                .rev()
                .find(|(time, _)| *time <= tick && *time > checkpoint)
            {
                cache.tick = *recent_tick;
                cache.states = recent_states.to_vec();
            } else {
                cache.tick = checkpoint;
                cache.states = states.to_vec();
            }
        }
        while cache.tick < tick {
            self.step(&mut cache.states);
            cache.tick += 1;
            if cache.tick.is_multiple_of(CHECKPOINT_TICKS) {
                cache
                    .checkpoints
                    .insert(cache.tick, Arc::from(cache.states.clone()));
                if cache.checkpoints.len() > MAX_CHECKPOINTS {
                    // Retain t=0 so arbitrary rewinds remain deterministic with bounded memory.
                    let oldest = *cache.checkpoints.keys().nth(1).unwrap();
                    cache.checkpoints.remove(&oldest);
                }
            }
        }
        // A collision substep can ask for t just before its interpolated endpoint.
        // Keep nearby ticks so float roundoff cannot trigger a two-second replay.
        if cache.recent.back().is_none_or(|(time, _)| *time != tick) {
            cache
                .recent
                .push_back((tick, Arc::from(cache.states.clone())));
            if cache.recent.len() > SAMPLE_CACHE {
                cache.recent.pop_front();
            }
        }
    }

    pub fn sample(&self, seconds: f32) -> Arc<[State]> {
        let seconds = if seconds.is_finite() {
            seconds.max(0.0)
        } else {
            0.0
        };
        let key = seconds.to_bits();
        let mut cache = self.cache.lock().expect("Debris trajectory cache poisoned");
        if let Some((_, states)) = cache.samples.iter().find(|(time, _)| *time == key) {
            return Arc::clone(states);
        }
        let continuous = seconds as f64 * RATE;
        let tick = continuous.floor() as u64;
        let fraction = (continuous - tick as f64) as f32;
        self.seek(&mut cache, tick);
        let mut result: Vec<_> = cache.states.iter().copied().map(Particle::state).collect();
        if fraction > 0.0 {
            self.seek(&mut cache, tick + 1);
            for (state, next) in result.iter_mut().zip(&cache.states) {
                state.position += (next.position.cast::<f32>() - state.position) * fraction;
                state.velocity += (next.velocity.cast::<f32>() - state.velocity) * fraction;
                state.captured |= next.captured;
            }
        }
        let result: Arc<[State]> = Arc::from(result);
        cache.samples.push_back((key, Arc::clone(&result)));
        if cache.samples.len() > SAMPLE_CACHE {
            cache.samples.pop_front();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::black_hole::BLACK_HOLE;

    fn body(distance: f32, radius: f32, velocity: Vec3) -> Body {
        Body {
            position: BLACK_HOLE.position + Vec3::x() * distance,
            radius,
            velocity,
        }
    }

    #[test]
    fn all_sizes_fall_equally_and_fast_rocks_are_captured_without_returning() {
        let field = Field::new(
            BLACK_HOLE,
            vec![
                body(500.0, 1.0, Vec3::zeros()),
                body(500.0, 15.0, Vec3::zeros()),
                body(100.0, 2.0, -Vec3::x() * 900.0),
            ],
        );
        let moved = field.sample(1.0);
        assert_eq!(moved[0].position, moved[1].position);
        assert_eq!(moved[0].velocity, moved[1].velocity);
        assert!(moved[0].position.x < BLACK_HOLE.position.x + 499.0);
        assert!(moved[2].captured);
        assert!(field.sample(30.0)[2].captured);
        assert_eq!(field.sample(30.0)[2].velocity, Vec3::zeros());
    }

    #[test]
    fn previews_rewind_repeat_and_do_not_depend_on_render_frame_rate() {
        let initial = vec![body(700.0, 3.0, Vec3::new(-4.0, 2.0, 15.0))];
        let field = Field::new(BLACK_HOLE, initial.clone());
        let original = field.sample(0.0);
        let before = field.sample(4.125);
        let later = field.sample(40.0);
        assert_eq!(&*field.sample(0.0), &*original);
        assert_eq!(&*field.sample(4.125), &*before);
        let other = Field::new(BLACK_HOLE, initial);
        for frame in 0..400 {
            other.sample(frame as f32 * 0.1);
        }
        assert_eq!(&*other.sample(40.0), &*later);
        assert_eq!(&*field.sample(f32::NAN), &*original);
    }

    #[test]
    fn tangential_velocity_curves_around_the_hole_without_numerical_energy_growth() {
        let radius = 800.0;
        let p = BLACK_HOLE.position + Vec3::x() * radius;
        let speed = (BLACK_HOLE.free_fall_acceleration(p).norm() * radius).sqrt();
        let field = Field::new(BLACK_HOLE, vec![body(radius, 2.0, Vec3::z() * speed)]);
        let initial_momentum = radius * speed;
        for seconds in [5.0, 20.0, 50.0, 150.0] {
            let state = field.sample(seconds)[0];
            assert!(!state.captured);
            let relative = state.position - BLACK_HOLE.position;
            assert!((relative.norm() - radius).abs() < 0.3);
            let momentum = relative.cross(&state.velocity).norm();
            assert!((momentum / initial_momentum - 1.0).abs() < 0.002);
        }
    }
}
