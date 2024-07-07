use std::ops::Deref;

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_ecs::{
    commands::Commands,
    component::{Component, Res, ResMut, Resource},
    prelude::UnsafeWorldCell,
    query::{Query, QueryFetch, QueryFetchItem, QueryFilter},
    system::{SystemAccess, SystemParam, SystemParamItem, SystemState},
    world::World,
};
use weaver_util::prelude::Result;

use crate::{
    ExtractBindGroupStage, ExtractPipelineStage, ExtractStage, MainWorld, ScratchMainWorld,
};

pub struct Extract<'w, 's, T: SystemParam> {
    item: SystemParamItem<'w, 's, T>,
}

impl<'w, 's, T: SystemParam> Deref for Extract<'w, 's, T> {
    type Target = SystemParamItem<'w, 's, T>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

pub struct ExtractState<T: SystemParam + 'static> {
    state: SystemState<T>,
    main_world_state: <Res<'static, MainWorld> as SystemParam>::State,
}

unsafe impl<T> SystemParam for Extract<'_, '_, T>
where
    T: SystemParam + 'static,
{
    type State = ExtractState<T>;
    type Item<'w, 's> = Extract<'w, 's, T>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: true,
            ..Default::default()
        }
    }

    fn validate_access(_access: &SystemAccess) -> bool {
        true
    }

    fn init_state(world: &mut World) -> Self::State {
        let main_world = world.get_resource_mut::<MainWorld>().unwrap().into_inner();
        ExtractState {
            state: SystemState::new(main_world),
            main_world_state: <Res<'_, MainWorld> as SystemParam>::init_state(world),
        }
    }

    unsafe fn fetch<'w, 's>(
        state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        let main_world = unsafe { Res::<MainWorld>::fetch(&mut state.main_world_state, world) };
        let item = state
            .state
            .get(main_world.into_inner().as_unsafe_world_cell_readonly());
        Extract { item }
    }

    fn can_run(world: &World) -> bool {
        if !<Res<'_, MainWorld> as SystemParam>::can_run(world) {
            log::debug!("Extract: Res<MainWorld> is not available");
            return false;
        }

        let main_world = world.get_resource::<MainWorld>().unwrap();
        if !<T as SystemParam>::can_run(&main_world) {
            log::debug!(
                "Extract: {} is not available in main world",
                std::any::type_name::<T>()
            );
            return false;
        }

        true
    }
}

pub trait ExtractComponent: Component {
    type ExtractQueryFetch: QueryFetch;
    type ExtractQueryFilter: QueryFilter;
    type Out: Component;
    fn extract_render_component(
        item: QueryFetchItem<'_, Self::ExtractQueryFetch>,
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
        render_app.add_system(extract_render_component::<T>, ExtractStage);
        Ok(())
    }
}

pub fn extract_render_component<T: ExtractComponent>(
    render_world: &mut World,
    query: Extract<Query<T::ExtractQueryFetch, T::ExtractQueryFilter>>,
) -> Result<()> {
    for (entity, item) in query.iter() {
        if let Some(component) = T::extract_render_component(item) {
            {
                if let Some(mut render_component) = render_world.get_component_mut::<T::Out>(entity)
                {
                    *render_component = component;
                    continue;
                }
            }

            log::trace!(
                "Extracted render component: {:?}",
                std::any::type_name::<T>()
            );

            render_world.insert_component(entity, component);
        }
    }

    Ok(())
}

pub trait ExtractResource: Resource {
    type Source: Resource;
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
        app.add_system(extract_render_resource::<T>, ExtractStage);
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
        app.add_system_after(
            extract_render_resource::<T>,
            extract_render_resource::<Dep>,
            ExtractStage,
        );
        Ok(())
    }
}

pub fn extract_render_resource<T: ExtractResource>(
    mut commands: Commands,
    main_resource: Extract<Option<Res<T::Source>>>,
    target_resource: Option<ResMut<T>>,
) -> Result<()> {
    if let Some(source) = main_resource.as_ref() {
        if let Some(mut target_resource) = target_resource {
            // update resource
            *target_resource = T::extract_render_resource(source);
        } else {
            commands.insert_resource(T::extract_render_resource(source));
            log::trace!(
                "Extracted render resource: {:?}",
                std::any::type_name::<T>()
            );
        }
    }

    Ok(())
}

pub fn render_extract(main_world: &mut World, render_world: &mut World) -> Result<()> {
    let scratch_world = main_world.remove_resource::<ScratchMainWorld>().unwrap();
    let inserted_world = std::mem::replace(main_world, scratch_world.0);
    render_world.insert_resource(MainWorld(inserted_world));

    render_world.run_stage::<ExtractStage>()?;
    render_world.run_stage::<ExtractBindGroupStage>()?;
    render_world.run_stage::<ExtractPipelineStage>()?;

    let inserted_world = render_world.remove_resource::<MainWorld>().unwrap();
    let scratch_world = std::mem::replace(main_world, inserted_world.0);
    main_world.insert_resource(ScratchMainWorld(scratch_world));

    Ok(())
}
