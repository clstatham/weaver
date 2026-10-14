//! Render-world entities that mirror main-world entities.
//!
//! The render world allocates its own entities. Each one that mirrors a main-world entity carries a
//! [`MainEntity`] component, and [`RenderEntities`] maps main-world entities to their render-world
//! counterparts. Mirrors are created on demand by extraction (see
//! [`RenderEntities::get_or_spawn`]) and despawned by [`despawn_orphaned_render_entities`] once
//! their main-world entity is gone.

use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    entity::{Entity, EntityMap},
};

use crate::MainWorld;

/// On a render-world entity: the main-world entity it mirrors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MainEntity(pub Entity);

/// Main-world entity -> its render-world mirror.
#[derive(Default)]
pub struct RenderEntities {
    map: EntityMap<Entity>,
}

impl RenderEntities {
    /// The render-world mirror of `main_entity`, if it has one.
    pub fn get(&self, main_entity: Entity) -> Option<Entity> {
        self.map.get(&main_entity).copied()
    }

    /// Returns the render-world mirror of `main_entity`, spawning it (with a [`MainEntity`]) if it
    /// doesn't exist yet. The new entity is usable with `commands` right away.
    pub fn get_or_spawn(&mut self, main_entity: Entity, commands: &Commands) -> Entity {
        *self
            .map
            .entry(main_entity)
            .or_insert_with(|| commands.spawn((MainEntity(main_entity),)))
    }

    /// Number of mirrored entities.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True if nothing is mirrored.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Despawns the mirrors of main-world entities that no longer exist.
pub fn despawn_orphaned_render_entities(
    commands: Commands,
    main_world: Res<MainWorld>,
    mut render_entities: ResMut<RenderEntities>,
) {
    render_entities
        .map
        .retain(|&main_entity, &mut render_entity| {
            let alive = main_world.is_alive(main_entity);
            if !alive {
                commands.despawn(render_entity);
            }
            alive
        });
}
