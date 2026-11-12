//! Swept ship spheres against the actual rotating planet meshes, including tunnels.
use crate::{model::Model, renderer::WorldTransform};
use nalgebra_glm::Vec3;
use std::sync::Arc;

/// The renderer normalizes every ship to the same bounding radius.
pub const SHIP_RADIUS: f32 = 1.7;
const SKIN: f32 = 0.003;

#[derive(Clone, Copy)]
struct Triangle([Vec3; 3]);

#[derive(Clone, Copy)]
struct Bounds {
    min: Vec3,
    max: Vec3,
}

enum Node {
    Leaf(Bounds, Vec<Triangle>),
    Branch(Bounds, Box<Node>, Box<Node>),
}

#[derive(Clone, Copy)]
struct Hit {
    time: f32,
    normal: Vec3,
    penetration: f32,
}

struct Sweep {
    start: Vec3,
    end: Vec3,
    radius: f32,
    time: f32,
    dt: f32,
}

#[derive(Clone)]
pub struct PlanetCollider {
    name: &'static str,
    transform: WorldTransform,
    rotation_speed: f32,
    motion: crate::orbit::Motion,
    debris: Option<(Arc<crate::debris_physics::Field>, usize)>,
    tree: Arc<Node>,
}

#[derive(Default)]
pub struct CollisionWorld {
    planets: Vec<PlanetCollider>,
}

#[derive(Clone, Copy)]
pub struct Contact {
    pub normal: Vec3,
    pub surface_velocity: Vec3,
    pub planet: &'static str,
}

pub struct Motion {
    pub position: Vec3,
    pub contacts: Vec<Contact>,
}

fn rotate(v: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(v.x * c + v.z * s, v.y, -v.x * s + v.z * c)
}

impl Bounds {
    fn intersects_segment(self, start: Vec3, delta: Vec3) -> bool {
        let (mut near, mut far) = (0.0f32, 1.0f32);
        for axis in 0..3 {
            if delta[axis].abs() < 1e-10 {
                if start[axis] < self.min[axis] || start[axis] > self.max[axis] {
                    return false;
                }
            } else {
                let a = (self.min[axis] - start[axis]) / delta[axis];
                let b = (self.max[axis] - start[axis]) / delta[axis];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far {
                    return false;
                }
            }
        }
        true
    }
    fn overlaps(self, other: Self) -> bool {
        (0..3).all(|i| self.min[i] <= other.max[i] && self.max[i] >= other.min[i])
    }
}

impl Node {
    fn blocks_light(&self, start: Vec3, delta: Vec3) -> bool {
        let bounds = match self {
            Self::Leaf(bounds, _) | Self::Branch(bounds, _, _) => *bounds,
        };
        if !bounds.intersects_segment(start, delta) {
            return false;
        }
        match self {
            Self::Leaf(_, triangles) => triangles
                .iter()
                .any(|triangle| triangle.blocks_light(start, delta)),
            Self::Branch(_, a, b) => a.blocks_light(start, delta) || b.blocks_light(start, delta),
        }
    }
    fn build(triangles: &mut [Triangle]) -> Self {
        let mut bounds = Bounds {
            min: Vec3::repeat(f32::INFINITY),
            max: Vec3::repeat(f32::NEG_INFINITY),
        };
        for p in triangles.iter().flat_map(|t| t.0) {
            bounds.min = bounds.min.zip_map(&p, f32::min);
            bounds.max = bounds.max.zip_map(&p, f32::max);
        }
        if triangles.len() <= 8 {
            return Self::Leaf(bounds, triangles.to_vec());
        }
        let size = bounds.max - bounds.min;
        let axis = if size.x >= size.y && size.x >= size.z {
            0
        } else if size.y >= size.z {
            1
        } else {
            2
        };
        triangles.sort_unstable_by(|a, b| {
            let center = |t: &Triangle| t.0.iter().map(|v| v[axis]).sum::<f32>();
            center(a).total_cmp(&center(b))
        });
        let middle = triangles.len() / 2;
        let (a, b) = triangles.split_at_mut(middle);
        Self::Branch(bounds, Box::new(Self::build(a)), Box::new(Self::build(b)))
    }

    fn sweep(&self, start: Vec3, delta: Vec3, radius: f32, query: Bounds, best: &mut Option<Hit>) {
        let bounds = match self {
            Self::Leaf(b, _) | Self::Branch(b, _, _) => *b,
        };
        if !bounds.overlaps(query) {
            return;
        }
        match self {
            Self::Leaf(_, triangles) => {
                for triangle in triangles {
                    if let Some(hit) = triangle.sweep(start, delta, radius)
                        && best.is_none_or(|old| {
                            hit.time < old.time
                                || (hit.time == old.time && hit.penetration > old.penetration)
                        })
                    {
                        *best = Some(hit);
                    }
                }
            }
            Self::Branch(_, a, b) => {
                a.sweep(start, delta, radius, query, best);
                b.sweep(start, delta, radius, query, best);
            }
        }
    }
}

/// Closest point on a triangle, including degenerate Boolean slivers.
fn closest_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let edge = b - a;
    a + edge * ((p - a).dot(&edge) / edge.norm_squared().max(1e-20)).clamp(0.0, 1.0)
}

impl Triangle {
    fn blocks_light(&self, start: Vec3, delta: Vec3) -> bool {
        let [a, b, c] = self.0;
        let edge_a = b - a;
        let edge_b = c - a;
        let h = delta.cross(&edge_b);
        let determinant = edge_a.dot(&h);
        if determinant.abs() < 1e-9 {
            return false;
        }
        let inverse = 1.0 / determinant;
        let relative = start - a;
        let u = relative.dot(&h) * inverse;
        if !(0.0..=1.0).contains(&u) {
            return false;
        }
        let q = relative.cross(&edge_a);
        let v = delta.dot(&q) * inverse;
        if v < 0.0 || u + v > 1.0 {
            return false;
        }
        let time = edge_b.dot(&q) * inverse;
        (0.000001..0.999999).contains(&time)
    }
    fn contains(&self, point: Vec3, n: Vec3) -> bool {
        (0..3).all(|i| {
            (self.0[(i + 1) % 3] - self.0[i])
                .cross(&(point - self.0[i]))
                .dot(&n)
                >= -1e-7
        })
    }

    fn closest(&self, point: Vec3) -> Vec3 {
        let [a, b, c] = self.0;
        let normal = (b - a).cross(&(c - a));
        if normal.norm_squared() > 1e-18 {
            let n = normal.normalize();
            let projected = point - n * (point - a).dot(&n);
            if self.contains(projected, n) {
                return projected;
            }
        }
        (0..3)
            .map(|i| closest_segment(point, self.0[i], self.0[(i + 1) % 3]))
            .min_by(|a, b| {
                (point - a)
                    .norm_squared()
                    .total_cmp(&(point - b).norm_squared())
            })
            .unwrap()
    }

    fn sweep(&self, start: Vec3, delta: Vec3, radius: f32) -> Option<Hit> {
        let offset = start - self.closest(start);
        let distance = offset.norm();
        let face_normal = (self.0[1] - self.0[0]).cross(&(self.0[2] - self.0[0]));
        let normal = if distance > 1e-8 {
            offset / distance
        } else if face_normal.norm_squared() > 1e-18 {
            face_normal.normalize()
        } else {
            Vec3::y()
        };
        if distance < radius - 1e-6 || (distance <= radius + 1e-6 && delta.dot(&normal) < -1e-8) {
            return Some(Hit {
                time: 0.0,
                normal,
                penetration: (radius - distance).max(0.0),
            });
        }
        let mut best: Option<Hit> = None;
        let mut add = |time: f32, normal: Vec3| {
            if (0.0..=1.0).contains(&time)
                && delta.dot(&normal) < -1e-8
                && best.is_none_or(|hit| time < hit.time)
            {
                best = Some(Hit {
                    time,
                    normal,
                    penetration: 0.0,
                });
            }
        };
        if face_normal.norm_squared() > 1e-18 {
            let n = face_normal.normalize();
            let speed = delta.dot(&n);
            if speed.abs() > 1e-10 {
                for sign in [-1.0, 1.0] {
                    let t = (sign * radius - (start - self.0[0]).dot(&n)) / speed;
                    if self.contains(start + delta * t - n * (sign * radius), n) {
                        add(t, n * sign);
                    }
                }
            }
        }
        for i in 0..3 {
            let a = self.0[i];
            let edge = self.0[(i + 1) % 3] - a;
            let length = edge.norm_squared();
            if length > 1e-18 {
                let o = start - a;
                let w = o - edge * (o.dot(&edge) / length);
                let v = delta - edge * (delta.dot(&edge) / length);
                if let Some(t) = first_root(
                    v.norm_squared(),
                    2.0 * w.dot(&v),
                    w.norm_squared() - radius * radius,
                ) {
                    let p = start + delta * t;
                    let u = (p - a).dot(&edge) / length;
                    if (0.0..=1.0).contains(&u) {
                        add(t, (p - a - edge * u).normalize());
                    }
                }
            }
            let o = start - a;
            if let Some(t) = first_root(
                delta.norm_squared(),
                2.0 * o.dot(&delta),
                o.norm_squared() - radius * radius,
            ) {
                add(t, (start + delta * t - a).normalize());
            }
        }
        best
    }
}

fn first_root(a: f32, b: f32, c: f32) -> Option<f32> {
    let d = b * b - 4.0 * a * c;
    if a <= 1e-18 || d < 0.0 {
        return None;
    }
    let t = (-b - d.sqrt()) / (2.0 * a);
    (0.0..=1.0).contains(&t).then_some(t)
}

impl PlanetCollider {
    pub fn blocks_model_light(&self, start: Vec3, end: Vec3) -> bool {
        self.tree.blocks_light(start, end - start)
    }
    pub fn new(
        name: &'static str,
        model: &Model,
        transform: WorldTransform,
        rotation_speed: f32,
    ) -> Self {
        Self::build(name, model, transform, rotation_speed, false)
    }

    pub fn new_opaque(
        name: &'static str,
        model: &Model,
        transform: WorldTransform,
        rotation_speed: f32,
    ) -> Self {
        Self::build(name, model, transform, rotation_speed, true)
    }

    fn build(
        name: &'static str,
        model: &Model,
        transform: WorldTransform,
        rotation_speed: f32,
        opaque_only: bool,
    ) -> Self {
        let mut triangles: Vec<_> = model
            .faces
            .iter()
            .filter(|face| !opaque_only || face.opacity >= 1.0)
            .map(|face| {
                Triangle(
                    face.indices
                        .map(|i| (model.vertices[i] - model.center) / model.radius),
                )
            })
            .collect();
        Self {
            name,
            transform,
            rotation_speed,
            motion: crate::orbit::Motion::default(),
            debris: None,
            tree: Arc::new(Node::build(&mut triangles)),
        }
    }

    /// Reuse normalized mesh geometry across differently sized, independently rotating instances.
    pub fn instance(&self, transform: WorldTransform, rotation_speed: f32) -> Self {
        Self {
            name: self.name,
            transform,
            rotation_speed,
            motion: self.motion,
            debris: self.debris.clone(),
            tree: Arc::clone(&self.tree),
        }
    }

    pub fn with_motion(mut self, motion: crate::orbit::Motion) -> Self {
        self.motion = motion;
        self
    }

    pub fn with_debris(mut self, field: Arc<crate::debris_physics::Field>, index: usize) -> Self {
        self.debris = Some((field, index));
        self
    }

    fn captured(&self, time: f32) -> bool {
        self.debris
            .as_ref()
            .is_some_and(|(field, index)| field.sample(time)[*index].captured)
    }

    fn center(&self, time: f32) -> Vec3 {
        self.debris.as_ref().map_or_else(
            || self.transform.position + self.motion.displacement(time),
            |(field, index)| field.sample(time)[*index].position,
        )
    }

    fn surface_velocity(&self, position: Vec3, time: f32) -> Vec3 {
        self.debris.as_ref().map_or_else(
            || self.motion.velocity(time),
            |(field, index)| field.sample(time)[*index].velocity,
        ) + Vec3::y().cross(&(position - self.center(time))) * self.rotation_speed
    }

    fn local(&self, point: Vec3, time: f32, center: Vec3) -> Vec3 {
        rotate(
            point - center,
            -(self.transform.yaw + self.rotation_speed * time),
        ) / self.transform.radius
    }

    fn hit(
        &self,
        sweep: &Sweep,
        debris: Option<(crate::debris_physics::State, crate::debris_physics::State)>,
    ) -> Option<Hit> {
        let Sweep {
            start,
            end,
            radius,
            time,
            dt,
        } = *sweep;
        if debris.is_some_and(|(state, _)| state.captured) {
            return None;
        }
        let center_a = debris.map_or_else(|| self.center(time), |(a, _)| a.position);
        let center_b = debris.map_or_else(|| self.center(time + dt), |(_, b)| b.position);
        let a = self.local(start, time, center_a);
        let b = self.local(end, time + dt, center_b);
        let r = radius / self.transform.radius;
        let query = Bounds {
            min: a.zip_map(&b, f32::min) - Vec3::repeat(r),
            max: a.zip_map(&b, f32::max) + Vec3::repeat(r),
        };
        let mut hit = None;
        self.tree.sweep(a, b - a, r, query, &mut hit);
        hit.map(|h| Hit {
            time: h.time,
            normal: rotate(
                h.normal,
                self.transform.yaw + self.rotation_speed * (time + h.time * dt),
            ),
            penetration: h.penetration * self.transform.radius,
        })
    }
}

impl CollisionWorld {
    /// Share immutable BVHs while removing the destroyed body and its ring debris.
    pub fn without_planet(&self, name: &str) -> Self {
        Self::new(
            self.planets
                .iter()
                .filter(|p| {
                    p.name != name && !(name == "Umbriel" && p.name == "Anillos de Umbriel")
                })
                .cloned()
                .collect(),
        )
    }
    pub fn celestial_acceleration(&self, position: Vec3, time: f32) -> Vec3 {
        let mut snapshots = Vec::new();
        let mut total = Vec3::zeros();
        for body in &self.planets {
            let center = if let Some((field, index)) = &body.debris {
                let key = Arc::as_ptr(field) as usize;
                if !snapshots.iter().any(|(id, _)| *id == key) {
                    snapshots.push((key, field.sample(time)));
                }
                let (_, states) = snapshots.iter().find(|(id, _)| *id == key).unwrap();
                if states[*index].captured {
                    continue;
                }
                states[*index].position
            } else {
                body.center(time)
            };
            total += crate::planet_gravity::attraction(center, body.transform.radius, position);
        }
        crate::planet_gravity::limit(total)
    }

    pub fn new(planets: Vec<PlanetCollider>) -> Self {
        Self { planets }
    }

    /// Floating portals need a clear disk and room for the ship at their exit.
    /// Conservative bounding spheres also reject portals buried inside a planet.
    pub fn sphere_clear(&self, center: Vec3, radius: f32, time: f32) -> bool {
        self.planets.iter().all(|planet| {
            planet.captured(time)
                || (planet.center(time) - center).norm() > planet.transform.radius + radius
        })
    }

    pub fn move_ship(&self, start: Vec3, displacement: Vec3, time: f32, dt: f32) -> Motion {
        let mut position = start;
        let mut remaining = displacement;
        let mut contacts = Vec::new();
        let mut time = time;
        let mut duration = dt;
        for _ in 0..6 {
            // Share the two immutable snapshots across all rocks, rather than
            // acquiring the simulation mutex separately for every mesh triangle query.
            let mut snapshots = Vec::new();
            for planet in &self.planets {
                if let Some((field, _)) = &planet.debris {
                    let key = Arc::as_ptr(field) as usize;
                    if !snapshots.iter().any(|(id, _, _)| *id == key) {
                        snapshots.push((key, field.sample(time), field.sample(time + duration)));
                    }
                }
            }
            let sweep = Sweep {
                start: position,
                end: position + remaining,
                radius: SHIP_RADIUS,
                time,
                dt: duration,
            };
            let hit = self
                .planets
                .iter()
                .filter_map(|planet| {
                    let debris = planet.debris.as_ref().map(|(field, index)| {
                        let (_, start, end) = snapshots
                            .iter()
                            .find(|(id, _, _)| *id == Arc::as_ptr(field) as usize)
                            .unwrap();
                        (start[*index], end[*index])
                    });
                    planet.hit(&sweep, debris).map(|h| (planet, h))
                })
                .min_by(|a, b| a.1.time.total_cmp(&b.1.time));
            let Some((planet, hit)) = hit else {
                position += remaining;
                break;
            };
            position += remaining * hit.time + hit.normal * (hit.penetration + SKIN);
            let surface_velocity = planet.surface_velocity(position, time + duration * hit.time);
            contacts.push(Contact {
                normal: hit.normal,
                surface_velocity,
                planet: planet.name,
            });
            time += duration * hit.time;
            duration *= 1.0 - hit.time;
            remaining *= 1.0 - hit.time;
            let relative = remaining - surface_velocity * duration;
            let inward = relative.dot(&hit.normal).min(0.0);
            remaining -= hit.normal * inward;
        }
        Motion { position, contacts }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_falling_asteroid_hits_a_stationary_ship_at_ten_and_sixty_fps() {
        let models = crate::asteroids::load().unwrap();
        for fps in [10, 60] {
            let transform = WorldTransform {
                position: Vec3::new(0.0, 0.0, -50.0),
                radius: 8.0,
                yaw: 0.0,
            };
            let field = Arc::new(crate::debris_physics::Field::new(
                crate::black_hole::BLACK_HOLE,
                vec![crate::debris_physics::Body {
                    position: transform.position,
                    radius: 8.0,
                    velocity: Vec3::z() * 80.0,
                }],
            ));
            let world = Arc::new(CollisionWorld::new(vec![
                models[4]
                    .collision
                    .instance(transform, 0.0)
                    .with_debris(field, 0),
            ]));
            let mut flight = crate::flight::Flight::with_collisions(world);
            flight.position = Vec3::zeros();
            for _ in 0..fps {
                flight.advance(1.0 / fps as f32, (0.0, 0.0), false, false);
                if flight.crash.is_some() {
                    break;
                }
            }
            assert_eq!(
                flight.crash.as_ref().map(|c| c.planet),
                Some("Asteroide grande")
            );
        }
    }

    #[test]
    fn captured_rocks_disappear_from_collision_and_portal_clearance_queries() {
        let models = crate::asteroids::load().unwrap();
        let hole = crate::black_hole::BLACK_HOLE;
        let transform = WorldTransform {
            position: hole.position + Vec3::x() * 80.0,
            radius: 8.0,
            yaw: 0.0,
        };
        let field = Arc::new(crate::debris_physics::Field::new(
            hole,
            vec![crate::debris_physics::Body {
                position: transform.position,
                radius: 8.0,
                velocity: -Vec3::x() * 150.0,
            }],
        ));
        let world = CollisionWorld::new(vec![
            models[4]
                .collision
                .instance(transform, 0.0)
                .with_debris(Arc::clone(&field), 0),
        ]);
        assert!(!world.sphere_clear(transform.position, 1.0, 0.0));
        assert!(
            world
                .celestial_acceleration(transform.position + Vec3::z() * 12.0, 0.0)
                .z
                < 0.0
        );
        let state = field.sample(2.0)[0];
        assert!(state.captured);
        assert!(world.sphere_clear(state.position, 1.0, 2.0));
        assert_eq!(
            world.celestial_acceleration(state.position + Vec3::z() * 12.0, 2.0),
            Vec3::zeros()
        );
        assert!(
            world
                .move_ship(
                    state.position + Vec3::z() * 30.0,
                    -Vec3::z() * 60.0,
                    2.0,
                    0.1
                )
                .contacts
                .is_empty()
        );
    }

    #[test]
    fn swept_sphere_hits_faces_edges_and_vertices_without_tunneling() {
        let tri = Triangle([
            Vec3::zeros(),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
        ]);
        for point in [
            Vec3::new(1.0, 1.0, 10.0),
            Vec3::new(2.0, -0.3, 10.0),
            Vec3::new(-0.3, -0.3, 10.0),
        ] {
            let hit = tri.sweep(point, Vec3::new(0.0, 0.0, -20.0), 0.5).unwrap();
            assert!(hit.time > 0.45 && hit.time < 0.5);
            assert!(hit.normal.z > 0.5);
        }
        assert!(
            tri.sweep(Vec3::new(8.0, 8.0, 10.0), Vec3::new(0.0, 0.0, -20.0), 0.5)
                .is_none()
        );
    }

    #[test]
    fn contact_allows_moving_outward_and_resolves_initial_overlap() {
        let tri = Triangle([
            Vec3::zeros(),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
        ]);
        assert!(
            tri.sweep(Vec3::new(1.0, 1.0, 0.5), Vec3::z(), 0.5)
                .is_none()
        );
        let hit = tri
            .sweep(Vec3::new(1.0, 1.0, 0.4), Vec3::zeros(), 0.5)
            .unwrap();
        assert!((hit.penetration - 0.1).abs() < 1e-5);
    }
}
