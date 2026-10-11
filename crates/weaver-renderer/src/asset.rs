//! GPU versions of main-world assets.
//!
//! A [`RenderAsset`] is created from a main-world asset (its `Source`). Per frame:
//! 1. [`RenderStage::Extract`]: [`extract_render_assets`] reads the frame's [`AssetEvent`]s and
//!    clones only the source assets that were added or modified (no GPU work on the main thread).
//! 2. [`RenderStage::PrepareAssets`]: [`prepare_render_assets`] turns them into GPU assets, stored
//!    in [`RenderAssets<T>`] under the *source* asset's id, and drops removed ones.
//!
//! Render code looks GPU assets up with the main-world handle it already has (for example the
//! `Handle<Mesh>` extracted onto a render entity): `render_assets.get(handle)`.

use std::marker::PhantomData;

use weaver_app::{App, plugin::Plugin};
use weaver_asset::{Asset, AssetEvent, AssetEventKind, AssetId, Assets, Handle};
use weaver_ecs::{
    component::{Res, ResMut},
    query::QueryableItem,
    system::{SystemParam, SystemParamItem, SystemParamWrapper},
};
use weaver_event::EventRx;
use weaver_util::prelude::*;

use crate::{
    RenderStage, WgpuDevice, WgpuQueue,
    extract::{Extract, ExtractComponent, ExtractComponentPlugin},
};

/// Why a render asset couldn't be prepared.
pub enum PrepareAssetError<S> {
    /// Something it depends on isn't ready yet (for example a texture a material uses). The source
    /// is handed back and preparation is retried next frame.
    RetryNextFrame(S),
}

/// The GPU version of a main-world asset.
pub trait RenderAsset: Send + Sync + Sized + 'static {
    type Source: Asset + Clone;
    /// Render-world data needed to prepare the asset (for example other render assets).
    type Param: SystemParam + 'static;

    fn prepare(
        source: Self::Source,
        param: &mut SystemParamItem<Self::Param>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Self, PrepareAssetError<Self::Source>>;
}

/// Prepared render assets, keyed by the id of the main-world asset they were made from.
pub struct RenderAssets<T: RenderAsset> {
    assets: FxHashMap<AssetId, T>,
}

impl<T: RenderAsset> Default for RenderAssets<T> {
    fn default() -> Self {
        Self {
            assets: FxHashMap::default(),
        }
    }
}

impl<T: RenderAsset> RenderAssets<T> {
    pub fn get(&self, source: Handle<T::Source>) -> Option<&T> {
        self.assets.get(&source.id())
    }

    pub fn get_by_id(&self, source: AssetId) -> Option<&T> {
        self.assets.get(&source)
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }
}

/// Source assets extracted this frame, waiting for [`prepare_render_assets`]; also holds assets
/// whose preparation is being retried.
pub struct ExtractedAssets<T: RenderAsset> {
    /// Sources to prepare, with how many times preparing each has been attempted.
    changed: Vec<(AssetId, T::Source, u32)>,
    removed: Vec<AssetId>,
}

/// How many frames an asset may wait for its dependencies before a warning is logged.
const RETRY_WARNING_THRESHOLD: u32 = 120;

impl<T: RenderAsset> Default for ExtractedAssets<T> {
    fn default() -> Self {
        Self {
            changed: Vec::new(),
            removed: Vec::new(),
        }
    }
}

pub struct RenderAssetPlugin<T: RenderAsset>(PhantomData<T>);

impl<T: RenderAsset> Default for RenderAssetPlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: RenderAsset> Plugin for RenderAssetPlugin<T> {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.init_resource::<RenderAssets<T>>();
        render_app.init_resource::<ExtractedAssets<T>>();
        render_app.add_system(extract_render_assets::<T>, RenderStage::Extract);
        render_app.add_system(prepare_render_assets::<T>, RenderStage::PrepareAssets);
        Ok(())
    }
}

pub fn extract_render_assets<T: RenderAsset>(
    mut events: Extract<EventRx<AssetEvent<T::Source>>>,
    sources: Extract<Res<Assets<T::Source>>>,
    mut extracted: ResMut<ExtractedAssets<T>>,
) {
    for event in events.by_ref() {
        // a newer event for the same asset supersedes anything still pending for it
        extracted.changed.retain(|(id, _, _)| *id != event.id);
        match event.kind {
            AssetEventKind::Added | AssetEventKind::Modified => {
                // the asset may have been removed again since the event was sent
                if let Some(source) = sources.get(event.handle()) {
                    extracted.changed.push((event.id, source.clone(), 0));
                }
            }
            AssetEventKind::Removed => extracted.removed.push(event.id),
        }
    }
}

pub fn prepare_render_assets<T: RenderAsset>(
    mut extracted: ResMut<ExtractedAssets<T>>,
    mut render_assets: ResMut<RenderAssets<T>>,
    mut param: SystemParamWrapper<T::Param>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) {
    for id in std::mem::take(&mut extracted.removed) {
        render_assets.assets.remove(&id);
    }

    let mut retry = Vec::new();
    for (id, source, attempts) in std::mem::take(&mut extracted.changed) {
        match T::prepare(source, param.item_mut(), &device, &queue) {
            Ok(asset) => {
                render_assets.assets.insert(id, asset);
            }
            Err(PrepareAssetError::RetryNextFrame(source)) => {
                let attempts = attempts + 1;
                if attempts == RETRY_WARNING_THRESHOLD {
                    log::warn!(
                        "{} for asset {id:?} still can't be prepared after {attempts} frames; \
                         is something it depends on missing?",
                        std::any::type_name::<T>()
                    );
                }
                retry.push((id, source, attempts));
            }
        }
    }
    extracted.changed = retry;
}

/// Orders `prepare_render_assets::<T>` after `prepare_render_assets::<Dep>`, so assets that depend
/// on `Dep` assets usually find them ready in the same frame instead of retrying next frame.
/// Both [`RenderAssetPlugin`]s must be added (in any order).
pub fn prepare_after<T: RenderAsset, Dep: RenderAsset>(render_app: &mut App) {
    render_app.order_systems(
        prepare_render_assets::<Dep>,
        prepare_render_assets::<T>,
        RenderStage::PrepareAssets,
    );
}

/// Asset handles on main-world entities are copied to their render-world mirrors as-is, so render
/// code can look up [`RenderAssets`] with them.
impl<A: Asset> ExtractComponent for Handle<A> {
    type Data = &'static Handle<A>;
    type Filter = weaver_ecs::query::Changed<&'static Handle<A>>;
    type Out = Handle<A>;

    fn extract(handle: QueryableItem<'_, Self::Data>) -> Option<Self::Out> {
        Some(*handle)
    }
}

/// Extracts `Handle<A>` components onto render-world entities.
pub type ExtractHandlePlugin<A> = ExtractComponentPlugin<Handle<A>>;

#[cfg(test)]
mod tests {
    use weaver_app::SubApp;
    use weaver_asset::AssetApp;
    use weaver_ecs::world::World;

    use super::*;
    use crate::{ScratchMainWorld, extract::render_extract};

    #[derive(Clone, Debug, PartialEq)]
    struct Blob(u32);
    impl Asset for Blob {}

    struct GpuBlob;

    impl RenderAsset for GpuBlob {
        type Source = Blob;
        type Param = ();

        fn prepare(
            _: Blob,
            _: &mut (),
            _: &wgpu::Device,
            _: &wgpu::Queue,
        ) -> Result<Self, PrepareAssetError<Blob>> {
            unreachable!("this test doesn't run the prepare stage")
        }
    }

    /// Only assets that were added or modified are extracted, once each; removals are passed on.
    #[test]
    fn extraction_follows_asset_events() {
        let mut main = SubApp::new();
        main.add_asset::<Blob>();
        main.world().insert_resource(ScratchMainWorld::default());
        main.world_mut().initialize_systems();

        let mut render = World::new();
        render.init_resource::<ExtractedAssets<GpuBlob>>();
        render.push_manual_stage(RenderStage::Extract);
        render.add_system(extract_render_assets::<GpuBlob>, RenderStage::Extract);
        render.initialize_systems();

        let frame = |main: &mut SubApp, render: &mut World| {
            main.world_mut().update().unwrap();
            render_extract(main.world_mut(), render).unwrap();
            let mut extracted = render
                .get_resource_mut::<ExtractedAssets<GpuBlob>>()
                .unwrap();
            let changed = std::mem::take(&mut extracted.changed)
                .into_iter()
                .map(|(_, blob, _)| blob.0)
                .collect::<Vec<_>>();
            let removed = std::mem::take(&mut extracted.removed);
            (changed, removed)
        };

        let (a, b) = {
            let mut blobs = main.world().get_resource_mut::<Assets<Blob>>().unwrap();
            (blobs.insert(Blob(1)), blobs.insert(Blob(2)))
        };
        assert_eq!(frame(&mut main, &mut render), (vec![1, 2], vec![]));
        assert_eq!(frame(&mut main, &mut render), (vec![], vec![]));

        {
            let mut blobs = main.world().get_resource_mut::<Assets<Blob>>().unwrap();
            blobs.get_mut(a).unwrap().0 = 10;
            blobs.remove(b);
        }
        assert_eq!(frame(&mut main, &mut render), (vec![10], vec![b.id()]));
        assert_eq!(frame(&mut main, &mut render), (vec![], vec![]));
    }
}
