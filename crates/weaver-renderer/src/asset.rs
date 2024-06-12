use std::{collections::HashMap, sync::Arc};

use weaver_app::{plugin::Plugin, App};
use weaver_asset::{Asset, Assets, Handle, UntypedHandle};
use weaver_ecs::{prelude::Resource, system::SystemStage, world::World};

use crate::Renderer;

pub trait RenderAsset: Asset {
    type BaseAsset: Asset;

    fn extract_render_asset(
        base_asset: &Self::BaseAsset,
        world: &World,
        renderer: &Renderer,
    ) -> Option<Self>
    where
        Self: Sized;

    fn update_render_asset(
        &self,
        base_asset: &Self::BaseAsset,
        world: &World,
        renderer: &Renderer,
    ) -> anyhow::Result<()>
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
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(extract_render_asset::<T>, SystemStage::PreRender)?;
        app.add_system(update_render_asset::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn extract_render_asset<T: RenderAsset>(world: Arc<World>) -> anyhow::Result<()> {
    // query for handles to the base asset
    let query = world.query::<&Handle<T::BaseAsset>>();

    for (entity, handle) in query.iter() {
        let mut extracted_assets = world.get_resource_mut::<ExtractedRenderAssets>().unwrap();
        if extracted_assets.contains(&handle.into_untyped()) {
            if !world.has_component::<Handle<T>>(entity) {
                // if the asset has already been extracted, insert the render asset handle into the entity
                let render_handle = *extracted_assets.assets.get(&handle.into_untyped()).unwrap();
                drop(handle);
                let render_handle = Handle::<T>::try_from(render_handle).unwrap();

                world.insert_component(entity, render_handle);
            }
        } else {
            // if the asset has not been extracted yet, extract it
            let renderer = world
                .get_resource::<Renderer>()
                .expect("Renderer resource not present before extracting render asset");
            let assets = world.get_resource::<Assets>().unwrap();
            let base_asset = assets.get::<T::BaseAsset>(*handle).unwrap();
            if let Some(render_asset) = T::extract_render_asset(base_asset, &world, &renderer) {
                log::debug!("Extracted render asset: {:?}", std::any::type_name::<T>());

                // insert the render asset into the asset storage
                drop(assets);
                let mut assets = world.get_resource_mut::<Assets>().unwrap();
                let render_handle = assets.insert(render_asset, None);

                let untyped_handle = handle.into_untyped();
                drop(handle);

                // insert the render asset handle into the entity
                world.insert_component(entity, render_handle);

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

fn update_render_asset<T: RenderAsset>(world: Arc<World>) -> anyhow::Result<()> {
    let query = world.query::<(&Handle<T>, &Handle<T::BaseAsset>)>();

    for (_entity, (render_handle, base_handle)) in query.iter() {
        let assets = world.get_resource::<Assets>().unwrap();
        let render_asset = assets.get::<T>(*render_handle).unwrap();
        let base_asset = assets.get::<T::BaseAsset>(*base_handle).unwrap();
        render_asset.update_render_asset(
            base_asset,
            &world,
            &world.get_resource::<Renderer>().unwrap(),
        )?;
    }

    Ok(())
}
