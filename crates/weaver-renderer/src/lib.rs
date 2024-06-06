use std::sync::Arc;

use wgpu::{Device, Queue};

pub mod camera;
pub mod resource;
pub mod target;

pub struct Renderer {
    device: Arc<Device>,
    queue: Arc<Queue>,
}
