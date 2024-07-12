use weaver_app::{plugin::Plugin, App};
use weaver_asset::{Asset, AssetApp, Assets, Handle, UntypedHandle};
use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    prelude::Resource,
    query::Query,
    system::{SystemParam, SystemParamItem, SystemParamWrapper},
};
use weaver_util::{
    lock::{Lock, Read, Write},
    {FxHashMap, Result},
};

use crate::{extract::Extract, ExtractStage, WgpuDevice, WgpuQueue};

pub trait RenderAsset: Asset {
    type Source: Asset;
    type Param: SystemParam + 'static;

    fn extract_render_asset(
        source: &Self::Source,
        param: &mut SystemParamItem<Self::Param>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<Self>
    where
        Self: Sized;

    fn update_render_asset(
        &mut self,
        source: &Self::Source,
        param: &mut SystemParamItem<Self::Param>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()>
    where
        Self: Sized;
}

#[derive(Default, Resource)]
pub struct ExtractedRenderAssets {
    assets: Lock<FxHashMap<UntypedHandle, UntypedHandle>>,
}

impl ExtractedRenderAssets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, handle: UntypedHandle, render_handle: UntypedHandle) {
        self.assets.write().insert(handle, render_handle);
    }

    pub fn contains(&self, handle: &UntypedHandle) -> bool {
        self.assets.read().contains_key(handle)
    }

    pub fn read(&self) -> Read<FxHashMap<UntypedHandle, UntypedHandle>> {
        self.assets.read()
    }

    pub fn write(&self) -> Write<FxHashMap<UntypedHandle, UntypedHandle>> {
        self.assets.write()
    }
}

pub struct ExtractRenderAssetPlugin<T: RenderAsset>(std::marker::PhantomData<T>);

impl<T: RenderAsset> Default for ExtractRenderAssetPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: RenderAsset> Plugin for ExtractRenderAssetPlugin<T> {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_asset::<T>();
        render_app.add_system(extract_render_asset::<T>, ExtractStage);
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_render_asset<T: RenderAsset>(
    mut commands: Commands,
    main_world_assets: Extract<Res<'static, Assets<T::Source>>>,
    mut param: SystemParamWrapper<T::Param>,
    query: Extract<Query<&'static Handle<T::Source>>>,
    extracted_assets: Res<ExtractedRenderAssets>,
    mut render_assets: ResMut<Assets<T>>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    // query for handles to the base asset
    for (entity, handle) in query.iter() {
        if !extracted_assets.contains(&handle.into_untyped()) {
            // if the asset has not been extracted yet, extract it
            let base_asset = main_world_assets.get(*handle).unwrap();
            if let Some(render_asset) =
                T::extract_render_asset(&base_asset, param.item_mut(), &device, &queue)
            {
                log::trace!("Extracted render asset: {:?}", std::any::type_name::<T>());

                // insert the render asset into the asset storage
                let render_handle = render_assets.insert(render_asset);

                let untyped_handle = handle.into_untyped();

                // insert the render asset handle into the entity
                commands.insert_component(entity, render_handle);

                // mark the original asset as extracted
                extracted_assets.insert(untyped_handle, render_handle.into_untyped());
            } else {
                log::error!(
                    "Failed to extract render asset: {:?}",
                    std::any::type_name::<T>()
                );
            }
        } else {
            // if the asset has already been extracted, insert the render asset handle into the entity
            let extracted_assets = extracted_assets.read();
            let render_handle = extracted_assets.get(&handle.into_untyped()).unwrap();
            let render_handle = Handle::<T>::try_from(*render_handle).unwrap();

            // update the asset
            let base_asset = main_world_assets.get(*handle).unwrap();
            let mut render_asset = render_assets.get_mut(render_handle).unwrap();
            render_asset.update_render_asset(&base_asset, param.item_mut(), &device, &queue)?;

            commands.insert_component(entity, render_handle);
        }
    }

    Ok(())
}
