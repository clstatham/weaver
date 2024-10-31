use std::{
    any::Any,
    ops::{Deref, DerefMut},
};

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    entity::Entity,
    query::{Query, Queryable, QueryableItem},
    system::{IntoSystem, SystemAccess, SystemParam, SystemParamItem},
    world::World,
};
use weaver_util::Result;

use crate::{
    ExtractBindGroupStage, ExtractPipelineStage, ExtractStage, MainWorld, ScratchMainWorld,
};

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

impl<T: SystemParam> SystemParam for Extract<T> {
    type Item = Extract<T>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: true,
            ..Default::default()
        }
    }

    fn validate_access(_access: &SystemAccess) -> bool {
        true
    }

    fn fetch(world: &World) -> Self::Item {
        let main_world = world.get_resource::<MainWorld>().unwrap();
        let item = T::fetch(&main_world);
        Extract { item }
    }

    fn can_run(world: &World) -> bool {
        if !<Res<MainWorld> as SystemParam>::can_run(world) {
            log::debug!("Extract: Res<MainWorld> is not available");
            return false;
        }

        let main_world = world.get_resource::<MainWorld>().unwrap();
        if !<T as SystemParam>::can_run(&main_world) {
            log::debug!("Extract: {} is not available", std::any::type_name::<T>());
            return false;
        }

        true
    }
}

pub trait ExtractComponent: Any + Send + Sync {
    type ExtractQueryFetch: Queryable + 'static;
    type Out: Any + Send + Sync + 'static;
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
        render_app.add_system(extract_render_component::<T>, ExtractStage);
        Ok(())
    }
}

pub async fn extract_render_component<T: ExtractComponent>(
    mut commands: Commands,
    mut query: Extract<Query<(Entity, T::ExtractQueryFetch)>>,
    mut out_query: Query<&mut T::Out>,
) {
    let mut components = Vec::new();
    for (entity, item) in query.iter() {
        if let Some(component) = T::extract_render_component(item) {
            {
                if let Some(render_component) = out_query.get(entity) {
                    *render_component = component;
                    log::trace!("Updated render component: {:?}", std::any::type_name::<T>());
                    continue;
                }
            }

            log::trace!(
                "Extracted render component: {:?}",
                std::any::type_name::<T>()
            );

            components.push((entity, component));
        }
    }

    for (entity, component) in components {
        commands.insert_component(entity, component).await;
    }
}

pub trait ExtractResource: Any + Send + Sync {
    type Source: Any + Send + Sync;
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

pub async fn extract_render_resource<T: ExtractResource>(
    mut commands: Commands,
    main_resource: Extract<Option<Res<T::Source>>>,
    target_resource: Option<ResMut<T>>,
) {
    if let Some(source) = main_resource.as_ref() {
        if let Some(mut target_resource) = target_resource {
            // update resource
            *target_resource = T::extract_render_resource(source);
            log::trace!("Updated render resource: {:?}", std::any::type_name::<T>());
        } else {
            commands
                .insert_resource(T::extract_render_resource(source))
                .await;
            log::trace!(
                "Extracted render resource: {:?}",
                std::any::type_name::<T>()
            );
        }
    }
}

pub fn render_extract(main_world: &mut World, render_world: &mut World) -> Result<()> {
    let scratch_world = main_world.remove_resource::<ScratchMainWorld>().unwrap();
    let inserted_world = std::mem::replace(main_world, scratch_world.0);
    render_world.insert_resource(MainWorld(inserted_world));

    pollster::block_on(async {
        render_world.run_stage::<ExtractStage>().await?;
        render_world.run_stage::<ExtractBindGroupStage>().await?;
        render_world.run_stage::<ExtractPipelineStage>().await?;
        Ok::<_, weaver_util::Error>(())
    })?;

    let inserted_world = render_world.remove_resource::<MainWorld>().unwrap();
    let scratch_world = std::mem::replace(main_world, inserted_world.0);
    main_world.insert_resource(ScratchMainWorld(scratch_world));

    Ok(())
}
