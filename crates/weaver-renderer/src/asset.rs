use std::collections::HashMap;

use weaver_app::{plugin::Plugin, App};
use weaver_asset::{Asset, Assets, Handle, UntypedHandle};
use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    prelude::Resource,
    query::Query,
};
use weaver_util::prelude::Result;

use crate::{Extract, MainWorld, WgpuDevice, WgpuQueue};

pub trait RenderAsset: Asset {
    type BaseAsset: Asset;

    fn extract_render_asset(
        base_asset: &Self::BaseAsset,
        main_world_assets: &Assets,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<Self>
    where
        Self: Sized;

    fn update_render_asset(
        &mut self,
        base_asset: &Self::BaseAsset,
        main_world_assets: &Assets,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()>
    where
        Self: Sized;
}

#[derive(Default, Resource)]
pub struct ExtractedRenderAssets {
    assets: HashMap<UntypedHandle, UntypedHandle>,
}

impl ExtractedRenderAssets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, handle: UntypedHandle, render_handle: UntypedHandle) {
        self.assets.insert(handle, render_handle);
    }

    pub fn contains(&self, handle: &UntypedHandle) -> bool {
        self.assets.contains_key(handle)
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
        render_app.add_system(extract_render_asset::<T>, Extract);
        render_app.add_system_after(update_render_asset::<T>, extract_render_asset::<T>, Extract);
        Ok(())
    }
}

fn extract_render_asset<T: RenderAsset>(
    commands: Commands,
    main_world: Res<MainWorld>,
    mut extracted_assets: ResMut<ExtractedRenderAssets>,
    render_handles: Query<&Handle<T>>,
    mut render_assets: ResMut<Assets>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    // query for handles to the base asset
    let query = main_world.query::<&Handle<T::BaseAsset>>();

    for (entity, handle) in query.iter(&main_world) {
        if extracted_assets.contains(&handle.into_untyped()) {
            if render_handles.get(entity).is_none() {
                // if the asset has already been extracted, insert the render asset handle into the entity
                let render_handle = *extracted_assets.assets.get(&handle.into_untyped()).unwrap();
                drop(handle);
                let render_handle = Handle::<T>::try_from(render_handle).unwrap();
                commands.insert_component(entity, render_handle);
            }
        } else {
            // if the asset has not been extracted yet, extract it

            let main_world_assets = unsafe { main_world.get_resource_unsafe::<Assets>().unwrap() };
            let base_asset = main_world_assets.get::<T::BaseAsset>(*handle).unwrap();
            if let Some(render_asset) =
                T::extract_render_asset(&base_asset, &main_world_assets, &device, &queue)
            {
                log::debug!("Extracted render asset: {:?}", std::any::type_name::<T>());

                // insert the render asset into the asset storage
                let render_handle = render_assets.insert(render_asset);

                let untyped_handle = handle.into_untyped();
                drop(handle);

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
        }
    }

    Ok(())
}

fn update_render_asset<T: RenderAsset>(
    main_world: Res<MainWorld>,
    extracted_handles: Res<ExtractedRenderAssets>,
    render_assets: Res<Assets>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    let query = main_world.query::<&Handle<T::BaseAsset>>();

    for (_entity, base_handle) in query.iter(&main_world) {
        let main_assets = unsafe { main_world.get_resource_mut_unsafe::<Assets>().unwrap() };

        let render_handle = extracted_handles
            .assets
            .get(&base_handle.into_untyped())
            .unwrap();
        let render_handle = Handle::<T>::try_from(*render_handle).unwrap();

        let mut render_asset = render_assets.get_mut::<T>(render_handle).unwrap();
        let base_asset = main_assets.get::<T::BaseAsset>(*base_handle).unwrap();

        render_asset.update_render_asset(&base_asset, &main_assets, &device, &queue)?;
    }

    Ok(())
}
