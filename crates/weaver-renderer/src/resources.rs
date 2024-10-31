use std::ops::{Deref, DerefMut};

pub struct ActiveCommandEncoder {
    encoder: wgpu::CommandEncoder,
}

impl ActiveCommandEncoder {
    pub fn new(encoder: wgpu::CommandEncoder) -> Self {
        Self { encoder }
    }

    pub fn finish(self) -> wgpu::CommandBuffer {
        self.encoder.finish()
    }
}

impl Deref for ActiveCommandEncoder {
    type Target = wgpu::CommandEncoder;

    fn deref(&self) -> &Self::Target {
        &self.encoder
    }
}

impl DerefMut for ActiveCommandEncoder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.encoder
    }
}
