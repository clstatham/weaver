//! Copying data from the main world to the render world.
//!
//! Extraction runs in [`RenderStage::Extract`] with the main world available as [`MainWorld`].
//! [`Extract`] lets a render-world system read from it, [`ExtractComponent`] and
//! [`ExtractResource`] cover the two common cases, and [`render_extract`] is the function that sets
//! the whole thing up each frame.

use std::{
    any::TypeId,
    ops::{Deref, DerefMut},
};

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_ecs::{
    change_detection::{ChangeDetection, Tick, TickRange},
    commands::Commands,
    component::{Res, ResMut},
    entity::Entity,
    prelude::Component,
    query::{Query, Queryable, QueryableItem},
    removal::RemovedComponents,
    system::{SystemAccess, SystemParam, SystemParamItem},
    world::World,
};
use weaver_util::prelude::*;

use crate::{MainWorld, RenderStage, ScratchMainWorld, sync::RenderEntities};

/// A system parameter that fetches `T` from the main world instead of the render world.
///
/// Only works in [`RenderStage::Extract`] systems, where the main world is available. Change
/// detection inside it (`Changed<..>`, `is_changed()`) is relative to the system's previous
/// extract run.
///
/// # Examples
///
/// ```
/// use weaver_ecs::query::Query;
/// use weaver_renderer::extract::Extract;
///
/// struct Health(u32);
///
/// // Runs in the render world, but the query iterates the main world.
/// fn count_health(mut query: Extract<Query<&Health>>) {
///     let _n = query.iter().count();
/// }
/// ```
pub struct Extract<T: SystemParam> {
    item: SystemParamItem<T>,
}

impl<T: SystemParam> Deref for Extract<T> {
    type Target = SystemParamItem<T>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<T: SystemParam> DerefMut for Extract<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}

/// State for [`Extract`]: the inner parameter's state, plus the main-world change ticks of this
/// system's extract runs, so change detection inside `Extract` means "changed in the main world
/// since this system last extracted".
pub struct ExtractState<S> {
    inner: S,
    /// Main-world tick of the previous extract run, `None` before the second run.
    last_run: Option<Tick>,
    /// Main-world tick of the current extract run, set in `update_state`.
    this_run: Option<Tick>,
}

impl<T: SystemParam + 'static> SystemParam for Extract<T> {
    type Item = Extract<T>;
    type State = ExtractState<T::State>;

    fn access() -> SystemAccess {
        // T's own access is against the main world, which nothing else touches during extraction
        let mut access = SystemAccess::default();
        access.resources_read.insert(TypeId::of::<MainWorld>());
        access
    }

    // `init_state` runs against the render world (the world the system lives in). Parameters whose
    // state is tied to a world (`Commands`) therefore act on the render world, which is what
    // `apply` below relies on.
    fn init_state(world: &World) -> Self::State {
        ExtractState {
            inner: T::init_state(world),
            last_run: None,
            this_run: None,
        }
    }

    fn update_state(state: &mut Self::State, world: &World) {
        let main_world = world
            .get_resource::<MainWorld>()
            .expect("Extract used outside of the extract stage");
        state.last_run = state.this_run;
        state.this_run = Some(main_world.bump_change_tick());
        T::update_state(&mut state.inner, &main_world);
    }

    fn fetch(world: &World, state: &Self::State, _ticks: TickRange) -> Self::Item {
        let main_world = world.get_resource::<MainWorld>().unwrap();
        let this_run = state
            .this_run
            .expect("Extract::update_state must run before fetch");
        let ticks = match state.last_run {
            Some(last_run) => TickRange::new(last_run, this_run),
            None => TickRange::first_run(this_run),
        };
        let item = T::fetch(&main_world, &state.inner, ticks);
        Extract { item }
    }

    fn apply(state: &mut Self::State, world: &mut World) {
        T::apply(&mut state.inner, world);
    }

    fn can_run(world: &World) -> bool {
        if !<Res<MainWorld> as SystemParam>::can_run(world) {
            log::debug!("Extract: Res<MainWorld> is not available");
            return false;
        }

        let main_world = world.get_resource::<MainWorld>().unwrap();
        if !<T as SystemParam>::can_run(&main_world) {
            log::debug!("Extract: {} is not available", T::type_name());
            return false;
        }

        true
    }
}

/// A main-world component that gets copied (possibly transformed) into the render world.
///
/// For every main-world entity matching `(&Self, Data, Filter)`, [`extract_component`] calls
/// [`ExtractComponent::extract`] and puts the result on the entity's render-world mirror (see
/// [`crate::sync`]), replacing the previous value. Returning `None` removes it. When the
/// main-world `Self` is removed, so is the render-world `Out`.
///
/// Use `Filter = Changed<&'static Self>` to only re-extract when the component changed, or `()`
/// to re-extract every frame (needed when `Data` reads other components that may change).
///
/// # Examples
///
/// ```
/// use weaver_ecs::storage::Ref;
/// use weaver_renderer::extract::ExtractComponent;
///
/// struct Health(u32);
/// struct GpuHealth(f32);
///
/// impl ExtractComponent for Health {
///     type Data = &'static Health;
///     type Filter = weaver_ecs::query::Changed<&'static Health>;
///     type Out = GpuHealth;
///
///     fn extract(health: Ref<'_, Health>) -> Option<GpuHealth> {
///         Some(GpuHealth(health.0 as f32))
///     }
/// }
/// ```
pub trait ExtractComponent: Component {
    /// Main-world data to extract from.
    type Data: Queryable + 'static;
    /// Which rows to extract this frame.
    type Filter: Queryable + 'static;
    /// What ends up on the render-world entity.
    type Out: Component;

    /// Converts the main-world data into the render-world component, or `None` to remove it.
    fn extract(item: QueryableItem<'_, Self::Data>) -> Option<Self::Out>;
}

/// Adds [`extract_component`] for `T` to [`RenderStage::Extract`].
pub struct ExtractComponentPlugin<T: ExtractComponent>(std::marker::PhantomData<T>);

impl<T: ExtractComponent> Default for ExtractComponentPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: ExtractComponent> Plugin for ExtractComponentPlugin<T> {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_system(extract_component::<T>, RenderStage::Extract);
        Ok(())
    }
}

/// Extracts `T` from every matching main-world entity onto its render-world mirror, spawning the
/// mirror if needed, and removes [`ExtractComponent::Out`] where `T` was removed.
#[allow(clippy::type_complexity)]
pub fn extract_component<T: ExtractComponent>(
    commands: Commands,
    mut query: Extract<Query<(Entity, &T, T::Data, T::Filter)>>,
    removed: Extract<RemovedComponents<T>>,
    mut render_entities: ResMut<RenderEntities>,
    mut out_query: Query<&mut T::Out>,
) {
    for (main_entity, _, item, _) in query.iter() {
        let render_entity = render_entities.get_or_spawn(main_entity, &commands);
        match T::extract(item) {
            Some(component) => match out_query.get(render_entity) {
                Some(mut existing) => *existing = component,
                None => commands.insert_component(render_entity, component),
            },
            None => commands.remove_component::<T::Out>(render_entity),
        }
    }

    for main_entity in removed.iter() {
        if let Some(render_entity) = render_entities.get(*main_entity) {
            commands.remove_component::<T::Out>(render_entity);
        }
    }
}

/// A main-world resource that gets copied (possibly transformed) into the render world whenever it
/// changes, and removed from the render world when it's removed from the main world.
pub trait ExtractResource: Component {
    /// The main-world resource to copy from. May be the same type as `Self`.
    type Source: Component;
    /// Builds the render-world resource from the main-world one.
    fn extract_render_resource(source: &Self::Source) -> Self;
}

/// Adds [`extract_render_resource`] for `T` to [`RenderStage::Extract`].
pub struct ExtractResourcePlugin<T: ExtractResource>(std::marker::PhantomData<T>);

impl<T: ExtractResource> Default for ExtractResourcePlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: ExtractResource> Plugin for ExtractResourcePlugin<T> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(extract_render_resource::<T>, RenderStage::Extract);
        Ok(())
    }
}

/// Keeps the render-world resource `T` in sync with its main-world source: inserted when the source
/// appears, rebuilt when it changes, removed when it goes away.
pub fn extract_render_resource<T: ExtractResource>(
    commands: Commands,
    main_resource: Extract<Option<Res<T::Source>>>,
    target_resource: Option<ResMut<T>>,
) {
    match (main_resource.as_ref(), target_resource) {
        (Some(source), Some(mut target)) => {
            if source.is_changed() {
                *target = T::extract_render_resource(source);
            }
        }
        (Some(source), None) => {
            commands.insert_resource(T::extract_render_resource(source));
        }
        (None, Some(_)) => {
            commands.remove_resource::<T>();
        }
        (None, None) => {}
    }
}

/// Runs [`RenderStage::Extract`] on the render world with the main world swapped into it as
/// [`MainWorld`], then swaps it back. This is the render sub-app's extract function.
///
/// # Errors
///
/// Returns the error if the extract stage fails.
///
/// # Panics
///
/// Panics if the main world has no [`ScratchMainWorld`].
pub fn render_extract(main_world: &mut World, render_world: &mut World) -> Result<()> {
    let scratch_world = main_world.remove_resource::<ScratchMainWorld>().unwrap();
    let inserted_world = std::mem::replace(main_world, scratch_world.0);
    render_world.insert_resource(MainWorld(inserted_world));

    // FIXME: on Err the `?` skips the swap back, leaving the real main world stuck in MainWorld
    render_world.run_stage(RenderStage::Extract)?;

    let inserted_world = render_world.remove_resource::<MainWorld>().unwrap();
    let scratch_world = std::mem::replace(main_world, inserted_world.0);
    main_world.insert_resource(ScratchMainWorld(scratch_world));

    Ok(())
}

#[cfg(test)]
mod tests {
    use weaver_ecs::query::Changed;
    use weaver_event::{EventQueue, EventRx};

    use super::*;

    /// `Extract<EventRx<T>>` must read the main world's events. The scheduler calls `init_state`
    /// with the world the system runs in (the render world), so the receivers have to come from
    /// the main world some other way.
    #[test]
    fn extract_event_rx_reads_main_world_events() {
        let render_world = World::new();
        render_world.insert_resource(EventQueue::<u32>::new());
        let state = <Extract<EventRx<u32>> as SystemParam>::init_state(&render_world);

        let main_world = World::new();
        main_world.insert_resource(EventQueue::<u32>::new());
        main_world
            .get_resource_mut::<EventQueue<u32>>()
            .unwrap()
            .send(7);
        render_world.insert_resource(MainWorld(main_world));

        let mut state = state;
        <Extract<EventRx<u32>> as SystemParam>::update_state(&mut state, &render_world);
        let ticks = TickRange::first_run(render_world.bump_change_tick());
        let mut rx = <Extract<EventRx<u32>> as SystemParam>::fetch(&render_world, &state, ticks);
        let mut received = Vec::new();
        for event in rx.by_ref() {
            received.push(event);
        }

        assert_eq!(received, vec![7]);
    }

    struct A(u32);

    fn extract_changed(render_world: &World, state: &mut ExtractState<()>) -> Vec<u32> {
        <Extract<Query<Changed<&A>>> as SystemParam>::update_state(state, render_world);
        let ticks = TickRange::first_run(render_world.bump_change_tick());
        let mut query =
            <Extract<Query<Changed<&A>>> as SystemParam>::fetch(render_world, state, ticks);
        let mut seen: Vec<u32> = query.iter().map(|a| a.0).collect();
        seen.sort();
        seen
    }

    /// `Extract<Query<Changed<..>>>` reports main-world changes made since the previous extract,
    /// each exactly once.
    #[test]
    fn extract_sees_main_world_changes_since_last_extract() {
        let main_world = World::new();
        let entities: Vec<Entity> = (0..3).map(|i| main_world.spawn((A(i),))).collect();
        let render_world = World::new();
        render_world.insert_resource(MainWorld(main_world));
        let mut state = <Extract<Query<Changed<&A>>> as SystemParam>::init_state(&render_world);

        assert_eq!(extract_changed(&render_world, &mut state), vec![0, 1, 2]);
        assert_eq!(
            extract_changed(&render_world, &mut state),
            Vec::<u32>::new()
        );

        {
            // a main-world system mutating one entity
            let main_world = render_world.get_resource::<MainWorld>().unwrap();
            let ticks = TickRange::new(Tick::default(), main_world.bump_change_tick());
            let mut query = Query::<&mut A>::new_with_ticks(&main_world, ticks);
            query.get(entities[1]).unwrap().0 = 10;
        }

        assert_eq!(extract_changed(&render_world, &mut state), vec![10]);
        assert_eq!(
            extract_changed(&render_world, &mut state),
            Vec::<u32>::new()
        );
    }

    mod sync {
        use std::sync::atomic::{AtomicUsize, Ordering};

        use weaver_ecs::{change_detection::ChangeDetectionMut, system::IntoSystemConfig};

        use super::super::*;
        use crate::sync::{MainEntity, despawn_orphaned_render_entities};

        #[derive(Debug, PartialEq)]
        struct Source(u32);
        #[derive(Debug, PartialEq)]
        struct Extracted(u32);

        impl ExtractComponent for Source {
            type Data = &'static Source;
            type Filter = weaver_ecs::query::Changed<&'static Source>;
            type Out = Extracted;

            fn extract(source: QueryableItem<'_, Self::Data>) -> Option<Self::Out> {
                Some(Extracted(source.0))
            }
        }

        /// Only used by `unchanged_components_are_not_re_extracted`, so its counter isn't shared.
        struct Counted;
        static COUNTED_EXTRACT_CALLS: AtomicUsize = AtomicUsize::new(0);

        impl ExtractComponent for Counted {
            type Data = ();
            type Filter = weaver_ecs::query::Changed<&'static Counted>;
            type Out = Extracted;

            fn extract(_: ()) -> Option<Self::Out> {
                COUNTED_EXTRACT_CALLS.fetch_add(1, Ordering::SeqCst);
                Some(Extracted(0))
            }
        }

        struct Worlds {
            main: World,
            render: World,
        }

        impl Worlds {
            fn new<T: ExtractComponent>() -> Self {
                let main = World::new();
                main.insert_resource(ScratchMainWorld::default());
                let mut render = World::new();
                render.init_resource::<RenderEntities>();
                render.push_manual_stage(RenderStage::Extract);
                render.add_system(despawn_orphaned_render_entities, RenderStage::Extract);
                render.add_system(
                    extract_component::<T>.after(despawn_orphaned_render_entities),
                    RenderStage::Extract,
                );
                render.initialize_systems();
                Self { main, render }
            }

            /// One frame: a main-world update, then extraction.
            fn frame(&mut self) {
                self.main.update().unwrap();
                render_extract(&mut self.main, &mut self.render).unwrap();
            }

            /// (main entity, extracted value) for every render-world mirror.
            fn mirrored(&self) -> Vec<(Entity, Option<u32>)> {
                let mut query = self.render.query::<(&MainEntity, Option<&Extracted>)>();
                let mut mirrored: Vec<_> = query
                    .iter()
                    .map(|(main, extracted)| (main.0, extracted.map(|e| e.0)))
                    .collect();
                mirrored.sort_by_key(|(entity, _)| entity.as_u64());
                mirrored
            }
        }

        #[test]
        fn render_entities_follow_main_entities() {
            let mut worlds = Worlds::new::<Source>();
            let a = worlds.main.spawn((Source(1),));
            let b = worlds.main.spawn((Source(2),));
            worlds.frame();
            assert_eq!(worlds.mirrored(), vec![(a, Some(1)), (b, Some(2))]);

            // render-world entities are the render world's own, not the main world's ids
            let render_entities = worlds.render.get_resource::<RenderEntities>().unwrap();
            let render_a = render_entities.get(a).unwrap();
            assert!(worlds.render.is_alive(render_a));
            drop(render_entities);

            // changes are extracted
            worlds.main.query::<&mut Source>().get(a).unwrap().0 = 10;
            worlds.frame();
            assert_eq!(worlds.mirrored(), vec![(a, Some(10)), (b, Some(2))]);

            // removing the component removes the extracted one, but keeps the mirror
            worlds.main.remove_component::<Source>(b);
            worlds.frame();
            assert_eq!(worlds.mirrored(), vec![(a, Some(10)), (b, None)]);

            // despawning despawns the mirror
            worlds.main.destroy_entity(a);
            worlds.frame();
            assert_eq!(worlds.mirrored(), vec![(b, None)]);
            assert!(!worlds.render.is_alive(render_a));
        }

        #[test]
        fn unchanged_components_are_not_re_extracted() {
            let mut worlds = Worlds::new::<Counted>();
            let entity = worlds.main.spawn((Counted,));
            for _ in 0..3 {
                worlds.frame();
            }
            assert_eq!(COUNTED_EXTRACT_CALLS.load(Ordering::SeqCst), 1);

            worlds
                .main
                .query::<&mut Counted>()
                .get(entity)
                .unwrap()
                .set_changed();
            worlds.frame();
            worlds.frame();
            assert_eq!(COUNTED_EXTRACT_CALLS.load(Ordering::SeqCst), 2);
        }
    }
}
