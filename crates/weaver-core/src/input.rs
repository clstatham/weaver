//! Keyboard and mouse state.
//!
//! [`Input`] is a resource that the winit runner pushes window and device events into. Game
//! systems read it with the `key_*` and `mouse_*` queries. [`InputPlugin`] clears the per-frame
//! parts (mouse delta, just-pressed edges) at the end of every frame.
//!
//! [`KeyCode`] and [`MouseButton`] are winit's types, re-exported here.

use std::collections::HashMap;

use weaver_app::{App, AppStage, plugin::Plugin};
use weaver_ecs::component::ResMut;
use weaver_util::prelude::*;
use winit::{
    event::{DeviceEvent, ElementState, WindowEvent},
    platform::scancode::PhysicalKeyExtScancode,
};

pub use winit::{event::MouseButton, keyboard::KeyCode};

/// Current keyboard and mouse state.
///
/// Keys are tracked by platform scancode, so a [`KeyCode`] means a physical key position and not
/// whatever the active keyboard layout prints on it. Mouse buttons are `Left`, `Right`, `Middle`
/// and `Other(0..=4)`; other buttons are ignored.
pub struct Input {
    /// Pressed state by scancode. A key that was never seen is absent.
    pub(crate) keys: HashMap<u32, bool>,
    /// Button state this frame, indexed Left, Right, Middle, then `Other(0..=4)`.
    pub(crate) mouse: [bool; 8],
    /// `mouse` as it was at the last [`Input::prepare`].
    pub(crate) mouse_last: [bool; 8],
    /// Cursor position in physical pixels, from the last `CursorMoved`.
    pub(crate) mouse_pos: (f32, f32),
    /// Raw mouse motion accumulated since the last [`Input::prepare`].
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
    /// Returns `true` if `key` is currently held.
    ///
    /// Keys with no scancode on this platform always read as not pressed.
    pub fn key_down(&self, key: KeyCode) -> bool {
        if let Some(scancode) = key.to_scancode() {
            self.keys.get(&scancode).copied().unwrap_or(false)
        } else {
            false
        }
    }

    /// Returns `true` if `key` is not held.
    pub fn key_up(&self, key: KeyCode) -> bool {
        !self.key_down(key)
    }

    /// Returns `true` if `button` is currently held.
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

    /// Returns `true` if `button` went down since the last [`Input::prepare`].
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

    /// Returns `true` if `button` went up since the last [`Input::prepare`].
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

    /// Returns `true` if `button` is not held.
    pub fn mouse_up(&self, button: winit::event::MouseButton) -> bool {
        !self.mouse_down(button)
    }

    /// Returns the cursor position in physical pixels.
    pub fn mouse_pos(&self) -> (f32, f32) {
        self.mouse_pos
    }

    /// Returns the raw mouse motion since the start of this frame.
    pub fn mouse_delta(&self) -> (f32, f32) {
        self.mouse_delta
    }

    /// Starts a new frame: zeroes the mouse delta and snapshots the button state for edge
    /// detection.
    pub fn prepare(&mut self) {
        self.mouse_delta = (0.0, 0.0);
        self.mouse_last = self.mouse;
    }

    /// Feeds in a device event; accumulates raw mouse motion.
    pub fn update_device(&mut self, event: &DeviceEvent) {
        #[allow(clippy::single_match)]
        match event {
            DeviceEvent::MouseMotion { delta } => {
                self.mouse_delta.0 += delta.0 as f32;
                self.mouse_delta.1 += delta.1 as f32;
            }
            _ => {}
        }
    }

    /// Feeds in a window event: cursor movement, mouse buttons and keyboard keys.
    pub fn update_window(&mut self, event: &WindowEvent) {
        // FIXME: only the latest state is kept, so a press and release between two frames is
        // never seen by key_down or mouse_just_pressed
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
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(scancode) = event.physical_key.to_scancode() {
                    self.keys
                        .insert(scancode, event.state == ElementState::Pressed);
                }
            }
            _ => {}
        }
    }
}

/// Inserts a default [`Input`] resource and calls [`Input::prepare`] on it every frame in
/// [`AppStage::PostUpdate`].
///
/// Something else (the winit runner) has to feed events into the resource.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.insert_resource(Input::default());
        app.add_system(update_input, AppStage::PostUpdate);
        Ok(())
    }
}

fn update_input(mut input: ResMut<Input>) {
    input.prepare();
}
