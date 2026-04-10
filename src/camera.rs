use std::f32::consts::FRAC_PI_2;
use glam::{Mat4, Vec3, Quat, EulerRot};
use winit::dpi::PhysicalPosition;
use winit::event::*;
use winit::keyboard::KeyCode;

use crate::model;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: Mat4 = Mat4::from_cols_array(&[
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
]);

const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_position: [f32; 4],
    view: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
    inv_proj: [[f32; 4]; 4],
    inv_view: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_position: [0.0; 4],
            view: Mat4::IDENTITY.to_cols_array_2d(),
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            inv_proj: Mat4::IDENTITY.to_cols_array_2d(),
            inv_view: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera, projection: &Projection) {
        self.view_position = camera.position.to_homogeneous().into();
        let proj = projection.calc_matrix();
        let view = camera.calc_matrix();
        let view_proj = proj * view;
        self.view = view.to_cols_array_2d();
        self.view_proj = view_proj.to_cols_array_2d();
        self.inv_proj = proj.inverse().to_cols_array_2d();
        self.inv_view = view.transpose().to_cols_array_2d();
    }
}

#[derive(Debug)]
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub rotation: Quat,
}

impl Camera {
    pub fn new<
        V: Into<Vec3>,
        Y: Into<f32>,
        P: Into<f32>,
        Roll: Into<f32>,
    >(
        position: V,
        yaw: Y,
        pitch: P,
        roll: Roll,
    ) -> Self {
        let yaw = yaw.into();
        let pitch = pitch.into();
        let roll = roll.into();
        Self {
            position: position.into(),
            yaw: yaw,
            pitch: pitch,
            rotation: Quat::from_euler(
                EulerRot::YXZ,
                yaw.to_radians(),
                pitch.to_radians(),
                roll.to_radians(),
            ),
        }
    }

    pub fn calc_matrix(&self) -> Mat4 {
        // Build the rotation quaternion from Euler angles in XYZ order
        // let rotation = Quat::from_euler(EulerRot::XYZ, self.pitch, self.yaw, self.roll);

        // Apply rotation to the default forward and up vectors
        let forward = (self.rotation * Vec3::Z).normalize();
        let up = (self.rotation * Vec3::Y).normalize();

        Mat4::look_to_rh(
            self.position,
            forward,
            up,
        )
    }
}

pub struct Projection {
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl Projection {
    pub fn new<F: Into<f32>>(
        width: u32,
        height: u32,
        fovy: F,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            aspect: width as f32 / height as f32,
            fovy: fovy.into(),
            znear,
            zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn calc_matrix(&self) -> Mat4 {
        OPENGL_TO_WGPU_MATRIX * glam::Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar)
    }
}

#[derive(Debug)]
pub struct CameraController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    mouse_dx: f32,
    mouse_dy: f32,
    mouse_x: f32,
    mouse_y: f32,
    scroll: f32,
    speed: f32,
    sensitivity: f32,
    aim_sensitivity: f32,
    is_free: bool,
    follow_smoothing: f32,
    rotation_smoothing: f32,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32, aim_sensitivity: f32, is_free: bool) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            scroll: 0.0,
            speed,
            sensitivity,
            aim_sensitivity,
            is_free: is_free,
            follow_smoothing: 5.0,
            rotation_smoothing: 2.0,
        }
    }

    pub fn handle_key(&mut self, key: KeyCode, pressed: bool) -> bool {
        if key == KeyCode::KeyV && pressed {
            // Make sure to reset any values so they don't stay active while in locked mode
            if self.is_free {
                self.reset_active_changes();
            }
            self.is_free = !self.is_free;
        }
        if self.is_free {
            let amount = if pressed { 1.0 } else { 0.0 };
            match key {
                KeyCode::KeyW | KeyCode::ArrowUp => {
                    self.amount_forward = amount;
                    return true;
                }
                KeyCode::KeyS | KeyCode::ArrowDown => {
                    self.amount_backward = amount;
                    return true;
                }
                KeyCode::KeyA | KeyCode::ArrowLeft => {
                    self.amount_left = amount;
                    return true;
                }
                KeyCode::KeyD | KeyCode::ArrowRight => {
                    self.amount_right = amount;
                    return true;
                }
                KeyCode::Space => {
                    self.amount_up = amount;
                    return true;
                }
                KeyCode::ShiftLeft => {
                    self.amount_down = amount;
                    return true;
                }
                _ => {
                    return false;
                }
            }
        } else {
            return false;
        }
    }

    fn reset_active_changes(&mut self) {
        self.amount_left = 0.0;
        self.amount_right = 0.0;
        self.amount_forward = 0.0;
        self.amount_backward = 0.0;
        self.amount_up = 0.0;
        self.amount_down = 0.0;
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
    }

    pub fn handle_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.mouse_dx = mouse_dx as f32;
        self.mouse_dy = mouse_dy as f32;
    }

    pub fn handle_scroll(&mut self, delta: &MouseScrollDelta) {
        if self.is_free {
            self.scroll = -match delta {
                // I'm assuming a line is about 100 pixels
                MouseScrollDelta::LineDelta(_, scroll) => scroll * 100.0,
                MouseScrollDelta::PixelDelta(PhysicalPosition {
                    y: scroll,
                    ..
                }) => *scroll as f32,
            };
        }
    }

    pub fn get_cursor_position(&self) -> (f32, f32) {
        (self.mouse_x, self.mouse_y)
    }

    pub fn set_cursor_position(&mut self, position: (f32, f32)) {
        (self.mouse_x, self.mouse_y) = position;
    }

    pub fn is_free(&self) -> bool {
        self.is_free
    }

    pub fn init_cursor_position(&mut self, width: u32, height: u32) {
        self.mouse_x = width as f32 * 0.5;
        self.mouse_y = height as f32 * 0.5;
    }

    pub fn clamp_cursor_position(&mut self, width: u32, height: u32) {
        self.mouse_x = self.mouse_x.clamp(0.0, width.saturating_sub(1) as f32);
        self.mouse_y = self.mouse_y.clamp(0.0, height.saturating_sub(1) as f32);
    }

    pub fn update_camera(&mut self, camera: &mut Camera, spaceship: &model::Instance, dt: f32) {
        if self.is_free {
            // Update position from keyboard
            let forward = camera.rotation * Vec3::Z;
            let right = camera.rotation * -Vec3::X;
            let move_fwd = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            let move_right = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
            camera.position += move_fwd * (self.amount_forward - self.amount_backward) * self.speed * dt;
            camera.position += move_right * (self.amount_right - self.amount_left) * self.speed * dt;
            camera.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

            // Update position from scrolling
            camera.position += -forward * self.scroll * self.speed * self.sensitivity * dt;
            self.scroll = 0.0;

            // Update raw angles
            camera.yaw += -self.mouse_dx * self.sensitivity * dt;
            camera.pitch += self.mouse_dy * self.sensitivity * dt;
            camera.pitch = camera.pitch.clamp(-SAFE_FRAC_PI_2, SAFE_FRAC_PI_2);

            // Re-generate the rotation Quat from the cleaned angles
            camera.rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0).normalize();

            self.mouse_dx = 0.0;
            self.mouse_dy = 0.0;
        } else {
            self.mouse_x += self.mouse_dx * (self.aim_sensitivity * 200.0) * dt;
            self.mouse_y -= self.mouse_dy * (self.aim_sensitivity * 200.0) * dt;

            // Calculate target position and rotation
            let local_offset = Vec3::new(0.0, 2.0, -20.0);
            let rotated_offset = spaceship.rotation * local_offset;
            let target_position = spaceship.position + rotated_offset;
            let target_rotation = spaceship.rotation;

            // Smoothly interpolate position towards target
            let t_position = 1.0 - (-self.follow_smoothing * dt).exp();
            camera.position = camera.position.lerp(target_position, t_position);

            // Smoothly interpolate rotation towards target
            let t_rotation = 1.0 - (-self.rotation_smoothing * dt).exp();
            camera.rotation = camera.rotation.slerp(target_rotation, t_rotation).normalize();

            self.mouse_dx = 0.0;
            self.mouse_dy = 0.0;
        }
    }
}