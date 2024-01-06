use rustc_hash::FxHashSet;
use weaver_proc_macro::Resource;
use winit::event::MouseButton;

pub use winit::event::VirtualKeyCode as KeyCode;

#[derive(Resource, Default)]
pub struct Input {
    keys_pressed: FxHashSet<KeyCode>,
    keys_held: FxHashSet<KeyCode>,
    mouse_buttons_pressed: FxHashSet<u32>,
    mouse_buttons_held: FxHashSet<u32>,
    mouse_position: Option<glam::Vec2>,
    mouse_delta: glam::Vec2,
    mouse_wheel_delta: f32,
}

impl Input {
    pub fn key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn key_pressed(&self, key: KeyCode) -> bool {
        self.keys_held.contains(&key)
    }

    pub fn mouse_button_just_pressed(&self, button: u32) -> bool {
        self.mouse_buttons_pressed.contains(&button)
    }

    pub fn mouse_button_pressed(&self, button: u32) -> bool {
        self.mouse_buttons_held.contains(&button)
    }

    pub fn mouse_position(&self) -> Option<glam::Vec2> {
        self.mouse_position
    }

    pub fn mouse_delta(&self) -> glam::Vec2 {
        self.mouse_delta
    }

    pub fn mouse_wheel_delta(&self) -> f32 {
        self.mouse_wheel_delta
    }

    pub fn prepare_for_update(&mut self) {
        self.keys_pressed.clear();
        self.mouse_buttons_pressed.clear();
        self.mouse_delta = glam::Vec2::ZERO;
        self.mouse_wheel_delta = 0.0;
    }

    pub fn update(&mut self, event: &winit::event::DeviceEvent) {
        match *event {
            winit::event::DeviceEvent::Key(input) => match input.state {
                winit::event::ElementState::Pressed => {
                    if !self.keys_held.contains(&input.virtual_keycode.unwrap()) {
                        self.keys_pressed.insert(input.virtual_keycode.unwrap());
                    }
                    self.keys_held.insert(input.virtual_keycode.unwrap());
                }
                winit::event::ElementState::Released => {
                    self.keys_held.remove(&input.virtual_keycode.unwrap());
                }
            },
            winit::event::DeviceEvent::MouseMotion { delta } => {
                self.mouse_delta = glam::Vec2::new(delta.0 as f32, delta.1 as f32);
            }
            winit::event::DeviceEvent::Button { button, state } => match state {
                winit::event::ElementState::Pressed => {
                    if !self.mouse_buttons_held.contains(&button) {
                        self.mouse_buttons_pressed.insert(button);
                    }
                    self.mouse_buttons_held.insert(button);
                }
                winit::event::ElementState::Released => {
                    self.mouse_buttons_held.remove(&button);
                }
            },
            winit::event::DeviceEvent::MouseWheel { delta } => match delta {
                winit::event::MouseScrollDelta::LineDelta(_, y) => {
                    self.mouse_wheel_delta = y;
                }
                winit::event::MouseScrollDelta::PixelDelta(_) => {}
            },
            _ => {}
        }
    }
}
