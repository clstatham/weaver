use std::{
    any::TypeId,
    ops::{Deref, DerefMut},
};

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    entity::Entity,
    prelude::Component,
    query::{Query, Queryable, QueryableItem},
    system::{IntoSystemConfig, SystemAccess, SystemParam, SystemParamItem},
    world::World,
};
use weaver_util::prelude::*;

use crate::{MainWorld, RenderStage, ScratchMainWorld};

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

impl<T: SystemParam + 'static> SystemParam for Extract<T> {
    type Item = Extract<T>;
    type State = T::State;

    fn access() -> SystemAccess {
        let mut access = SystemAccess::default();
        access.resources_read.insert(TypeId::of::<MainWorld>());
        access
    }

    fn init_state(world: &World) -> Self::State {
        T::init_state(world)
    }

    fn fetch(world: &World, state: &Self::State) -> Self::Item {
        let main_world = world.get_resource::<MainWorld>().unwrap();
        let item = T::fetch(&main_world, state);
        Extract { item }
    }

    // `state` was created against the render world in `init_state`, so deferred work applies there too
    fn apply(state: &mut Self::State, world: &mut World) {
        T::apply(state, world);
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

pub trait ExtractComponent: Component {
    type ExtractQueryFetch: Queryable + 'static;
    type Out: Component + 'static;
    fn extract_render_component(
        item: QueryableItem<'_, Self::ExtractQueryFetch>,
    ) -> Option<Self::Out>;
}

pub struct ExtractComponentPlugin<T: ExtractComponent>(std::marker::PhantomData<T>);

impl<T: ExtractComponent> Default for ExtractComponentPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: ExtractComponent> Plugin for ExtractComponentPlugin<T> {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_system(extract_render_component::<T>, RenderStage::Extract);
        Ok(())
    }
}

pub fn extract_render_component<T: ExtractComponent>(
    commands: Commands,
    mut query: Extract<Query<(Entity, T::ExtractQueryFetch)>>,
    mut out_query: Query<&mut T::Out>,
) {
    let mut components = Vec::new();
    for (entity, item) in query.iter() {
        if let Some(component) = T::extract_render_component(item) {
            {
                if let Some(mut render_component) = out_query.get(entity) {
                    *render_component = component;
                    log::trace!("Updated render component: {:?}", T::type_name());
                    continue;
                }
            }

            log::trace!("Extracted render component: {:?}", T::type_name());

            components.push((entity, component));
        }
    }

    for (entity, component) in components {
        commands.insert_component(entity, component);
    }
}

pub trait ExtractResource: Component {
    type Source: Component;
    fn extract_render_resource(source: &Self::Source) -> Self;
}

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

pub struct RenderResourceDependencyPlugin<T: ExtractResource, Dep: ExtractResource>(
    std::marker::PhantomData<(T, Dep)>,
);

impl<T: ExtractResource, Dep: ExtractResource> Default for RenderResourceDependencyPlugin<T, Dep> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: ExtractResource, Dep: ExtractResource> Plugin for RenderResourceDependencyPlugin<T, Dep> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(
            extract_render_resource::<T>.after(extract_render_resource::<Dep>),
            RenderStage::Extract,
        );
        Ok(())
    }
}

pub fn extract_render_resource<T: ExtractResource>(
    commands: Commands,
    main_resource: Extract<Option<Res<T::Source>>>,
    target_resource: Option<ResMut<T>>,
) {
    if let Some(source) = main_resource.as_ref() {
        if let Some(mut target_resource) = target_resource {
            // update resource
            *target_resource = T::extract_render_resource(source);
            log::trace!("Updated render resource: {:?}", T::type_name());
        } else {
            commands.insert_resource(T::extract_render_resource(source));
            log::trace!("Extracted render resource: {:?}", T::type_name());
        }
    }
}

pub fn render_extract(main_world: &mut World, render_world: &mut World) -> Result<()> {
    let scratch_world = main_world.remove_resource::<ScratchMainWorld>().unwrap();
    let inserted_world = std::mem::replace(main_world, scratch_world.0);
    render_world.insert_resource(MainWorld(inserted_world));

    render_world.run_stage(RenderStage::Extract)?;
    render_world.run_stage(RenderStage::ExtractBindGroup)?;
    render_world.run_stage(RenderStage::ExtractPipeline)?;

    let inserted_world = render_world.remove_resource::<MainWorld>().unwrap();
    let scratch_world = std::mem::replace(main_world, inserted_world.0);
    main_world.insert_resource(ScratchMainWorld(scratch_world));

    Ok(())
}

#[cfg(test)]
mod tests {
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

        let mut rx = <Extract<EventRx<u32>> as SystemParam>::fetch(&render_world, &state);
        let mut received = Vec::new();
        for event in rx.by_ref() {
            received.push(event);
        }

        assert_eq!(received, vec![7]);
    }
}
