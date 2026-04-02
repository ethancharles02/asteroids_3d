use glam::{Vec3, Quat};
use web_time::{Duration};

use crate::{ModelInstances, camera, model, resources};

const SPACESHIP_MODEL_ID: usize = 0;
const ASTEROID_MODEL_ID: usize = 1;

pub struct Hitbox {
    center: Vec3,
    radii: Vec3,
}

pub struct GameObject {
    position: Vec3,
    velocity: Vec3,
    rotation: Vec3,
    hitbox: Hitbox,
}

pub struct GameManager {
    // objects: Vec<GameObject>,
    // game_models: Vec<Model>,
    timer: Duration,
    counter: u8,
}

impl GameManager {
    pub fn get_models(device: &wgpu::Device, queue: &wgpu::Queue, texture_bind_group_layout: &wgpu::BindGroupLayout) -> Vec<model::Model> {
        let spaceship_model =
            resources::load_model("Spaceship.obj", &device, &queue, &texture_bind_group_layout)
                .unwrap();
        let asteroid_model =
            resources::load_model("Asteroid.obj", &device, &queue, &texture_bind_group_layout)
                .unwrap();
        let models = vec![spaceship_model, asteroid_model];
        return models;
    }

    pub fn new(model_instances: &mut ModelInstances) -> GameManager {
        let spaceship = model::Instance {
            position: Vec3 { x: 0.0, y: 0.0, z: 0.0},
            rotation: Quat::from_axis_angle(Vec3::Z, 0.0_f32.to_radians()),
        };
        let asteroid = model::Instance {
            position: Vec3 { x: 0.0, y: -10.0, z: 0.0},
            rotation: Quat::from_axis_angle(Vec3::Z, 0.0_f32.to_radians()),
        };
        model_instances.add_instance(SPACESHIP_MODEL_ID, spaceship);
        model_instances.add_instance(ASTEROID_MODEL_ID, asteroid);
        GameManager { timer: Duration::new(0, 0), counter: 0 }
    }

    pub fn update(&mut self, dt: Duration, camera_controller: &mut camera::CameraController, camera: &mut camera::Camera, model_instances: &mut ModelInstances) {
        self.timer += dt;

        if let Some(instances) = model_instances.get_instances_from_model_id(SPACESHIP_MODEL_ID) {
            if let Some(_) = instances.get(0) {
                let mut_spaceship = model_instances.get_mut_instance(SPACESHIP_MODEL_ID, 0);
                mut_spaceship.position.x += 1.0 * dt.as_secs_f32();
                camera_controller.update_camera(camera, mut_spaceship, dt);
                // let speed = 60.0_f32.to_radians();
                // let rotate = Quat::from_axis_angle(Vec3::Y, speed * dt.as_secs_f32());
                // let spaceship = model_instances.get_mut_instance(0, 0);
                // spaceship.rotation = (rotate * spaceship.rotation).normalize();
            }
        }

        // TODO add dynamic asteroid creation and movement (don't delete asteroids? just move them when they go past the player?)
        let asteroid_speed = 10.0_f32.to_radians();
        let asteroid_rotate = Quat::from_axis_angle(Vec3::Y, asteroid_speed * dt.as_secs_f32());
        let asteroid = model_instances.get_mut_instance(ASTEROID_MODEL_ID, 0);
        asteroid.rotation = (asteroid_rotate * asteroid.rotation).normalize();
    }
}