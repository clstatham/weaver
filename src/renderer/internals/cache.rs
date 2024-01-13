use std::sync::Arc;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use super::BindableComponent;

/// A cache for bind group layouts. This is used to avoid creating duplicate bind group layouts.
#[derive(Default)]
pub struct BindGroupLayoutCache {
    /// Bind group layouts for each component id.
    pub(crate) layouts: RwLock<FxHashMap<usize, Arc<wgpu::BindGroupLayout>>>,
}

impl BindGroupLayoutCache {
    /// Get the bind group layout for the given component type. If the layout does not exist, it is created using `T::create_bind_group_layout()`.
    pub fn get_or_create<T: BindableComponent>(
        &self,
        device: &wgpu::Device,
    ) -> Arc<wgpu::BindGroupLayout> {
        // check if the layout already exists
        let id = T::static_id();
        if let Some(layout) = self.layouts.read().get(&id) {
            // return the existing layout
            return layout.clone();
        }

        // create the layout
        let layout = T::create_bind_group_layout(device);
        self.layouts.write().insert(id, Arc::new(layout));
        self.layouts.read().get(&id).unwrap().clone()
    }
}
