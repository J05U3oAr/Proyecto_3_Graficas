mod camera;
mod color;
mod framebuffer;
mod model;
mod triangle;

use camera::Camera;
use color::Color;
use framebuffer::Framebuffer;
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use model::{Face, Model};
use nalgebra_glm::Vec3;
use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;
use triangle::{fill, line};

const WIDTH: usize = 1100;
const HEIGHT: usize = 800;

struct Ship {
    name: &'static str,
    model: Model,
    camera: Camera,
}

#[derive(Clone, Copy)]
enum RenderMode {
    Solid,
    Wireframe,
    Hologram,
}

impl RenderMode {
    fn next(self) -> Self {
        match self {
            Self::Solid => Self::Wireframe,
            Self::Wireframe => Self::Hologram,
            Self::Hologram => Self::Solid,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Solid => "Sólido",
            Self::Wireframe => "Malla",
            Self::Hologram => "Holograma",
        }
    }
}

fn asset_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets/ships")
        .join(filename)
}

fn render(framebuffer: &mut Framebuffer, ship: &Ship, mode: RenderMode) {
    framebuffer.clear();

    let (right, up, forward) = ship.camera.basis();
    let scale = 0.82 * framebuffer.width.min(framebuffer.height) as f32 / (2.0 * ship.model.radius)
        * ship.camera.zoom;
    let center_x = framebuffer.width as f32 * 0.5;
    let center_y = framebuffer.height as f32 * 0.5;

    let view_vertices: Vec<Vec3> = ship
        .model
        .vertices
        .iter()
        .map(|vertex| {
            let relative = vertex - ship.model.center;
            Vec3::new(
                center_x + relative.dot(&right) * scale,
                center_y - relative.dot(&up) * scale,
                -relative.dot(&forward),
            )
        })
        .collect();

    let light = Vec3::new(-0.4, 0.8, 1.0).normalize();
    for face in &ship.model.faces {
        let Face { indices, color } = *face;
        let a = &view_vertices[indices[0]];
        let b = &view_vertices[indices[1]];
        let c = &view_vertices[indices[2]];

        match mode {
            RenderMode::Solid => {
                let mut normal = (b - a).cross(&(c - a));
                if normal.norm_squared() < f32::EPSILON {
                    continue;
                }
                normal = normal.normalize();
                if normal.z < 0.0 {
                    normal = -normal;
                }
                let diffuse = normal.dot(&light).max(0.0);
                fill(framebuffer, a, b, c, color.shaded(0.28 + 0.92 * diffuse));
            }
            RenderMode::Wireframe => {
                let edge_color = Color::new(105, 195, 235).to_hex();
                line(framebuffer, a, b, edge_color, true);
                line(framebuffer, b, c, edge_color, true);
                line(framebuffer, c, a, edge_color, true);
            }
            RenderMode::Hologram => {
                let edge_color = Color::new(35, 205, 255).to_hex();
                line(framebuffer, a, b, edge_color, false);
                line(framebuffer, b, c, edge_color, false);
                line(framebuffer, c, a, edge_color, false);
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut ships = vec![
        Ship {
            name: "Guardgalaxy",
            model: Model::load(asset_path("Guardgalaxy.obj"))?,
            camera: Camera::new(),
        },
        Ship {
            name: "Galactwar",
            model: Model::load(asset_path("Galactwar.obj"))?,
            camera: Camera::new(),
        },
        Ship {
            name: "Lightflash",
            model: Model::load(asset_path("Lightflash.obj"))?,
            camera: Camera::new(),
        },
    ];

    for ship in &ships {
        println!(
            "{}: {} vértices, {} triángulos",
            ship.name,
            ship.model.vertices.len(),
            ship.model.faces.len()
        );
    }

    let mut selected = 0usize;
    let mut mode = RenderMode::Solid;
    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    let mut window = Window::new(
        "Proyecto de gráficas | 1-3: nave | Flechas: orbitar | M: modo",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )?;
    window.set_target_fps(60);
    let mut dirty = true;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if window.is_key_pressed(Key::Key1, KeyRepeat::No) {
            selected = 0;
            dirty = true;
        } else if window.is_key_pressed(Key::Key2, KeyRepeat::No) {
            selected = 1;
            dirty = true;
        } else if window.is_key_pressed(Key::Key3, KeyRepeat::No) {
            selected = 2;
            dirty = true;
        }

        if window.is_key_pressed(Key::M, KeyRepeat::No) {
            mode = mode.next();
            dirty = true;
        }

        let ship = &mut ships[selected];
        if window.is_key_down(Key::Left) || window.is_key_down(Key::A) {
            ship.camera.orbit(-0.035, 0.0);
            dirty = true;
        }
        if window.is_key_down(Key::Right) || window.is_key_down(Key::D) {
            ship.camera.orbit(0.035, 0.0);
            dirty = true;
        }
        if window.is_key_down(Key::Up) || window.is_key_down(Key::W) {
            ship.camera.orbit(0.0, 0.035);
            dirty = true;
        }
        if window.is_key_down(Key::Down) || window.is_key_down(Key::S) {
            ship.camera.orbit(0.0, -0.035);
            dirty = true;
        }
        if window.is_key_down(Key::E) {
            ship.camera.zoom = (ship.camera.zoom * 1.015).min(3.0);
            dirty = true;
        }
        if window.is_key_down(Key::Q) {
            ship.camera.zoom = (ship.camera.zoom / 1.015).max(0.45);
            dirty = true;
        }
        if window.is_key_pressed(Key::R, KeyRepeat::No) {
            ship.camera = Camera::new();
            dirty = true;
        }

        if dirty {
            render(&mut framebuffer, ship, mode);
            window.set_title(&format!(
                "{} | {} | 1-3: nave | Flechas: orbitar | Q/E: zoom | M: modo | R: reiniciar",
                ship.name,
                mode.label()
            ));
            dirty = false;
        }

        if window.is_key_pressed(Key::P, KeyRepeat::No) {
            framebuffer.save_png("render.png")?;
            println!("Captura guardada en render.png");
        }

        window.update_with_buffer(&framebuffer.pixels, WIDTH, HEIGHT)?;
        std::thread::sleep(Duration::from_millis(8));
    }

    Ok(())
}
