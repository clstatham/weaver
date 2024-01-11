use std::{cell::RefCell, sync::Arc};

use rustc_hash::FxHashMap;

use super::BindableComponent;

#[derive(Default)]
pub struct BindGroupLayoutCache {
    /// Bind group layouts for each component id.
    pub(crate) layouts: RefCell<FxHashMap<u64, Arc<wgpu::BindGroupLayout>>>,
}

impl BindGroupLayoutCache {
    pub fn get_or_create<T: BindableComponent>(
        &self,
        device: &wgpu::Device,
    ) -> Arc<wgpu::BindGroupLayout> {
        let id = T::component_id();
        if let Some(layout) = self.layouts.borrow().get(&id) {
            return layout.clone();
        }

        let layout = T::create_bind_group_layout(device);
        self.layouts.borrow_mut().insert(id, Arc::new(layout));
        self.layouts.borrow().get(&id).unwrap().clone()
    }
}
