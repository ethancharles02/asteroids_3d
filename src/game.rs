use glam::{Vec3, Quat};
use winit::{
    keyboard::{KeyCode},
};

use crate::{camera, model, resources};

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
    timer: f32,
    player_droll: f32,
    player_cur_droll: f32,
    player_dspeed: f32,
    player_cur_dspeed: f32,
    player_speed: f32,
    player_min_max_speed: (f32, f32),
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

    pub fn new(model_instances: &mut model::ModelInstances, player_roll_sensitivity: f32, player_min_max_speed: (f32, f32), player_dspeed: f32) -> GameManager {
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
        GameManager { timer: 0.0, player_droll: player_roll_sensitivity, player_cur_droll: 0.0, player_cur_dspeed: 0.0, player_dspeed, player_speed: player_min_max_speed.0, player_min_max_speed }
    }

    pub fn update(&mut self, config: &wgpu::SurfaceConfiguration, dt: f32, camera_controller: &mut camera::CameraController, camera: &mut camera::Camera, model_instances: &mut model::ModelInstances) {
        self.timer += dt;

        if let Some(instances) = model_instances.get_instances_from_model_id(SPACESHIP_MODEL_ID) {
            if let Some(_) = instances.get(0) {
                let mut_spaceship = model_instances.get_mut_instance(SPACESHIP_MODEL_ID, 0);

                self.player_speed = (self.player_speed + self.player_cur_dspeed * dt).clamp(self.player_min_max_speed.0, self.player_min_max_speed.1);
                let forward = mut_spaceship.rotation * Vec3::Z;
                mut_spaceship.position += forward * self.player_speed * dt;

                // Update rotation of the ship
                let droll_rotation = Quat::from_rotation_z(self.player_cur_droll * dt);
                let width_f32 = config.width as f32;
                let height_f32 = config.height as f32;
                let cursor_position = camera_controller.get_cursor_position();
                let rel_mouse_x = cursor_position.0 - (width_f32 / 2.0);
                let rel_mouse_y = cursor_position.1 - (height_f32 / 2.0);
                let yaw_rotation = Quat::from_rotation_y(-(rel_mouse_x / width_f32) * dt);
                let pitch_rotation = Quat::from_rotation_x(-(rel_mouse_y / height_f32) * dt);
                mut_spaceship.rotation = mut_spaceship.rotation * droll_rotation * yaw_rotation * pitch_rotation;
                mut_spaceship.rotation = mut_spaceship.rotation.normalize();

                camera_controller.update_camera(camera, mut_spaceship, dt);
                camera_controller.clamp_cursor_position(config.width, config.height);
            }
        }

        // TODO add dynamic asteroid creation and movement (don't delete asteroids? just move them when they go past the player?)
        let asteroid_speed = 10.0_f32.to_radians();
        let asteroid_rotate = Quat::from_axis_angle(Vec3::Y, asteroid_speed * dt);
        let asteroid = model_instances.get_mut_instance(ASTEROID_MODEL_ID, 0);
        asteroid.rotation = (asteroid_rotate * asteroid.rotation).normalize();
    }

    pub fn reset_active_changes(&mut self) {
        self.player_cur_droll = 0.0;
    }

    pub fn handle_key(&mut self, key: winit::keyboard::KeyCode, pressed: bool, camera_controller: &mut camera::CameraController) -> bool {
        let mut key_handled = true;
        if !camera_controller.handle_key(key, pressed) {
            let amount: f32 = if pressed { 1.0 } else { 0.0 };
            match key {
                KeyCode::KeyA => self.player_cur_droll = amount * -self.player_droll,
                KeyCode::KeyD => self.player_cur_droll = amount * self.player_droll,
                KeyCode::KeyW => self.player_cur_dspeed = amount * self.player_dspeed,
                KeyCode::KeyS => self.player_cur_dspeed = amount * -self.player_dspeed,
                _ => {key_handled = false}
            }
        }
        if key == KeyCode::KeyV && pressed {
            self.reset_active_changes();
        }
        return key_handled;
    }
}