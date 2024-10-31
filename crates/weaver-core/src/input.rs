use std::collections::HashMap;

use weaver_app::{plugin::Plugin, App, PostUpdate};
use weaver_ecs::component::ResMut;
use weaver_util::Result;
use winit::{
    event::{DeviceEvent, ElementState, WindowEvent},
    platform::scancode::PhysicalKeyExtScancode,
};

pub use winit::{event::MouseButton, keyboard::KeyCode};

pub struct Input {
    pub(crate) keys: HashMap<u32, bool>,
    pub(crate) mouse: [bool; 8],
    pub(crate) mouse_last: [bool; 8],
    pub(crate) mouse_pos: (f32, f32),
    pub(crate) mouse_delta: (f32, f32),
}

impl Default for Input {
    fn default() -> Self {
        Self {
            keys: HashMap::new(),
            mouse: [false; 8],
            mouse_last: [false; 8],
            mouse_pos: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
        }
    }
}

impl Input {
    pub fn key_down(&self, key: KeyCode) -> bool {
        if let Some(scancode) = key.to_scancode() {
            self.keys.get(&scancode).copied().unwrap_or(false)
        } else {
            false
        }
    }

    pub fn key_up(&self, key: KeyCode) -> bool {
        !self.key_down(key)
    }

    pub fn mouse_down(&self, button: MouseButton) -> bool {
        match button {
            MouseButton::Left => self.mouse[0],
            MouseButton::Right => self.mouse[1],
            MouseButton::Middle => self.mouse[2],
            MouseButton::Other(0) => self.mouse[3],
            MouseButton::Other(1) => self.mouse[4],
            MouseButton::Other(2) => self.mouse[5],
            MouseButton::Other(3) => self.mouse[6],
            MouseButton::Other(4) => self.mouse[7],
            _ => false,
        }
    }

    pub fn mouse_just_pressed(&self, button: MouseButton) -> bool {
        let index = match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Other(0) => 3,
            MouseButton::Other(1) => 4,
            MouseButton::Other(2) => 5,
            MouseButton::Other(3) => 6,
            MouseButton::Other(4) => 7,
            _ => return false,
        };

        self.mouse[index] && !self.mouse_last[index]
    }

    pub fn mouse_just_released(&self, button: MouseButton) -> bool {
        let index = match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Other(0) => 3,
            MouseButton::Other(1) => 4,
            MouseButton::Other(2) => 5,
            MouseButton::Other(3) => 6,
            MouseButton::Other(4) => 7,
            _ => return false,
        };

        !self.mouse[index] && self.mouse_last[index]
    }

    pub fn mouse_up(&self, button: winit::event::MouseButton) -> bool {
        !self.mouse_down(button)
    }

    pub fn mouse_pos(&self) -> (f32, f32) {
        self.mouse_pos
    }

    pub fn mouse_delta(&self) -> (f32, f32) {
        self.mouse_delta
    }

    pub fn prepare(&mut self) {
        self.mouse_delta = (0.0, 0.0);
        self.mouse_last = self.mouse;
    }

    pub fn update_device(&mut self, event: &DeviceEvent) {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                self.mouse_delta.0 += delta.0 as f32;
                self.mouse_delta.1 += delta.1 as f32;
            }
            DeviceEvent::Key(key) => {
                if let Some(scancode) = key.physical_key.to_scancode() {
                    self.keys
                        .insert(scancode, key.state == ElementState::Pressed);
                }
            }
            _ => {}
        }
    }

    pub fn update_window(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput { button, state, .. } => match button {
                MouseButton::Left => self.mouse[0] = *state == ElementState::Pressed,
                MouseButton::Right => self.mouse[1] = *state == ElementState::Pressed,
                MouseButton::Middle => self.mouse[2] = *state == ElementState::Pressed,
                MouseButton::Other(0) => self.mouse[3] = *state == ElementState::Pressed,
                MouseButton::Other(1) => self.mouse[4] = *state == ElementState::Pressed,
                MouseButton::Other(2) => self.mouse[5] = *state == ElementState::Pressed,
                MouseButton::Other(3) => self.mouse[6] = *state == ElementState::Pressed,
                MouseButton::Other(4) => self.mouse[7] = *state == ElementState::Pressed,
                _ => {}
            },
            _ => {}
        }
    }
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.insert_resource(Input::default());
        app.add_system(update_input, PostUpdate);
        Ok(())
    }
}

async fn update_input(mut input: ResMut<Input>) {
    input.prepare();
}
