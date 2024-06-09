use std::collections::HashSet;

use weaver_app::{plugin::Plugin, App};
use weaver_asset::{Asset, Assets, Handle, UntypedHandle};
use weaver_ecs::{query::Query, system::SystemStage, world::World};

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
}

#[derive(Default)]
pub struct ExtractedRenderAssets {
    assets: HashSet<UntypedHandle>,
}

impl ExtractedRenderAssets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, handle: UntypedHandle) {
        self.assets.insert(handle);
    }

    pub fn contains(&self, handle: &UntypedHandle) -> bool {
        self.assets.contains(handle)
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
        app.add_resource(ExtractedRenderAssets::new());
        app.add_system(extract_render_asset::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn extract_render_asset<T: RenderAsset>(world: &World) -> anyhow::Result<()> {
    // query for handles to the base asset
    let query = world.query(&Query::new().read::<Handle<T::BaseAsset>>());

    for entity in query.iter() {
        let handle = query.get::<Handle<T::BaseAsset>>(entity).unwrap();

        // if the asset has not been extracted yet, extract it
        let mut extracted_assets = world.get_resource_mut::<ExtractedRenderAssets>().unwrap();
        if !extracted_assets.contains(&handle.into_untyped()) {
            let renderer = world
                .get_resource::<Renderer>()
                .expect("Renderer resource not present before extracting render asset");
            let mut assets = world.get_resource_mut::<Assets>().unwrap();
            let base_asset = assets.get::<T::BaseAsset>(*handle).unwrap();
            if let Some(render_asset) = T::extract_render_asset(base_asset, world, &renderer) {
                log::debug!("Extracted render asset: {:?}", std::any::type_name::<T>());

                // insert the render asset into the asset storage
                let render_handle = assets.insert(render_asset, None);

                let untyped_handle = handle.into_untyped();
                drop(handle);

                // insert the render asset handle into the entity
                world.insert_component(entity, render_handle);

                // mark the original asset as extracted
                extracted_assets.insert(untyped_handle);
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
