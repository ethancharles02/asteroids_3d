use glam::{Vec3, Quat};
use crate::{ModelInstances, camera, model};

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
    timer: instant::Duration,
    counter: u8,
}

impl GameManager {
    pub fn new() -> GameManager {
        GameManager { timer: instant::Duration::new(0, 0), counter: 0 }
    }

    pub fn update(&mut self, dt: instant::Duration, camera_controller: &mut camera::CameraController, camera: &mut camera::Camera, model_instances: &mut ModelInstances) {
        self.timer += dt;

        if let Some(instances) = model_instances.get_instances_from_model_id(0) {
            if let Some(_) = instances.get(0) {
                let mut_spaceship = model_instances.get_mut_instance(0, 0);
                mut_spaceship.position.x += 1.0 * dt.as_secs_f32();
                camera_controller.update_camera(camera, mut_spaceship, dt);
            }
        }

        // let speed = 60.0_f32.to_radians();
        // let rotate = Quat::from_axis_angle(Vec3::Y, speed * dt.as_secs_f32());
        // let spaceship = model_instances.get_mut_instance(0, 0);
        // spaceship.rotation = (rotate * spaceship.rotation).normalize();

        let asteroid_speed = 10.0_f32.to_radians();
        let asteroid_rotate = Quat::from_axis_angle(Vec3::Y, asteroid_speed * dt.as_secs_f32());
        let asteroid = model_instances.get_mut_instance(1, 0);
        asteroid.rotation = (asteroid_rotate * asteroid.rotation).normalize();

        // if self.timer.as_secs_f32() > 1.0 {
        //     if let Some(last_bucket) = model_instances.model_buckets.last() {
        //         model_instances.add_instance(1, model::Instance {
        //             position: Vec3 { x: 0.0, y: -10.0 * (last_bucket.start_index + last_bucket.current_len) as f32, z: 0.0},
        //             rotation: Quat::from_axis_angle(Vec3::Z, 0.0_f32.to_radians()),
        //         });
        //         self.timer = instant::Duration::new(0, 0);
        //         self.counter += 1;
        //     }
        // }

        // if self.counter == 10 {
        //     // let mut to_delete: Vec<usize> = Vec::new();
        //     // if let Some(asteroids) = model_instances.get_instances_from_model_id(1) {
        //     //     for (idx, _) in asteroids.iter().rev().enumerate() {
        //     //         to_delete.push(idx);
        //     //     }
        //     // }

        //     // for idx in to_delete {
        //     //     model_instances.remove_instance(1, idx);
        //     // }
        //     // TODO these attributes shouldn't be public
        //     for idx in (0..(model_instances.model_buckets[1].current_len)).rev() {
        //         model_instances.remove_instance(1, idx);
        //     }
        // }
    }
}