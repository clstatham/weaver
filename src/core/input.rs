use weaver_proc_macro::Resource;
use winit::event::MouseButton;

pub use winit::event::VirtualKeyCode as KeyCode;
use winit_input_helper::WinitInputHelper;

#[derive(Resource)]
pub struct Input {
    pub input: WinitInputHelper,
}

impl Input {
    pub fn new() -> Self {
        Self {
            input: WinitInputHelper::new(),
        }
    }

    pub fn key_pressed(&self, key: KeyCode) -> bool {
        self.input.key_held(key)
    }

    pub fn key_just_pressed(&self, key: KeyCode) -> bool {
        self.input.key_pressed(key)
    }

    pub fn mouse_button_pressed(&self, button: MouseButton) -> bool {
        let b = match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Other(_) => return false,
        };
        self.input.mouse_held(b)
    }

    pub fn mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        let b = match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Other(_) => return false,
        };
        self.input.mouse_pressed(b)
    }

    pub fn mouse_position(&self) -> Option<glam::Vec2> {
        self.input.mouse().map(|(x, y)| glam::Vec2::new(x, y))
    }

    pub fn mouse_delta(&self) -> glam::Vec2 {
        self.input.mouse_diff().into()
    }

    pub fn mouse_wheel_delta(&self) -> f32 {
        self.input.scroll_diff()
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}
