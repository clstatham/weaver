use rustc_hash::FxHashMap;
use winit::event::{Event, MouseButton, WindowEvent};

pub use winit::event::VirtualKeyCode as KeyCode;

use crate::ecs::resource::Resource;

#[derive(Default)]
pub struct Input {
    pub keys: FxHashMap<KeyCode, bool>,
    pub mouse_buttons: FxHashMap<MouseButton, bool>,
    pub mouse_position: glam::Vec2,
    pub mouse_delta: glam::Vec2,
    pub mouse_wheel_delta: f32,
}
impl Resource for Input {}

impl Input {
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys.get(&key).copied().unwrap_or(false)
    }

    pub fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons.get(&button).copied().unwrap_or(false)
    }

    pub fn mouse_position(&self) -> glam::Vec2 {
        self.mouse_position
    }

    pub fn mouse_delta(&self) -> glam::Vec2 {
        self.mouse_delta
    }

    pub fn mouse_wheel_delta(&self) -> f32 {
        self.mouse_wheel_delta
    }

    pub fn prepare(&mut self) {
        self.mouse_delta = glam::Vec2::ZERO;
        self.mouse_wheel_delta = 0.0;
    }

    pub fn update(&mut self, input: &Event<'_, ()>) {
        self.prepare();
        match input {
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                if let Some(key_code) = input.virtual_keycode {
                    self.keys
                        .insert(key_code, input.state == winit::event::ElementState::Pressed);
                }
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { button, state, .. },
                ..
            } => {
                self.mouse_buttons
                    .insert(*button, *state == winit::event::ElementState::Pressed);
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                let position = glam::Vec2::new(position.x as f32, position.y as f32);
                self.mouse_delta = position - self.mouse_position;
                self.mouse_position = position;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        delta: winit::event::MouseScrollDelta::LineDelta(_, y),
                        ..
                    },
                ..
            } => {
                self.mouse_wheel_delta = *y;
            }
            _ => {}
        }
    }
}
