use std::sync::Arc;

use crate::Renderer;

#[derive(Debug)]
pub enum RenderTarget {
    PrimaryScreen,
    TextureView(Arc<wgpu::TextureView>),
}

impl RenderTarget {
    pub fn texture_view(&self, renderer: &Renderer) -> Option<Arc<wgpu::TextureView>> {
        match self {
            Self::PrimaryScreen => renderer.current_frame_view(),
            Self::TextureView(view) => Some(view.clone()),
        }
    }
}
