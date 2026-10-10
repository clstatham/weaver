use std::ops::{Deref, DerefMut};

pub struct ActiveCommandEncoder {
    encoder: Option<wgpu::CommandEncoder>,
}

impl ActiveCommandEncoder {
    pub fn new(encoder: wgpu::CommandEncoder) -> Self {
        Self {
            encoder: Some(encoder),
        }
    }

    pub fn finish(&mut self) -> wgpu::CommandBuffer {
        self.encoder.take().unwrap().finish()
    }
}

impl Deref for ActiveCommandEncoder {
    type Target = wgpu::CommandEncoder;

    fn deref(&self) -> &Self::Target {
        self.encoder
            .as_ref()
            .expect("ActiveCommandEncoder has been finished")
    }
}

impl DerefMut for ActiveCommandEncoder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.encoder
            .as_mut()
            .expect("ActiveCommandEncoder has been finished")
    }
}
