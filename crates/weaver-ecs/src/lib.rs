//! A single-threaded, archetype-based entity component system, modeled on Bevy's.
//!
//! Everything hangs off a [`World`](world::World). Entities are generational ids ([`entity`]);
//! their components live in archetype tables, one table per distinct set of component types
//! ([`storage`]). Singleton data is stored as resources in the same world. Systems are plain
//! functions whose parameters say what they touch, grouped into stages and run in order by
//! [`World::update`](world::World::update).
//!
//! # Entities and components
//!
//! Any `Send + Sync + 'static` type is a [`Component`](component::Component). A
//! [`Bundle`](bundle::Bundle) (a tuple of components) is spawned or inserted as a unit. Entities
//! with the same component types share an archetype, so adding or removing a component moves the
//! entity to another table.
//! Resources are one-per-type values held by the world and fetched with
//! [`Res`](component::Res) and [`ResMut`](component::ResMut).
//!
//! # Queries
//!
//! A [`Query`](query::Query) iterates the entities that have a given set of components, with
//! `&T`, `&mut T`, [`With`](query::With), [`Without`](query::Without), `Option<_>` and
//! [`Changed`](query::Changed) as building blocks. A query holds per-type borrow flags for as
//! long as it lives. Reading and writing conflicts are detected at runtime, and so is any
//! structural change (spawning, despawning, inserting or removing components) attempted while a
//! query is alive: that panics.
//!
//! # Systems and stages
//!
//! A system is a function (or closure) whose arguments implement
//! [`SystemParam`](system::SystemParam): queries, `Res`/`ResMut`, [`Commands`](commands::Commands),
//! [`Local`](system::Local) state and so on. Systems are added to a stage, a label type that
//! derives [`SystemStage`]. Within a stage the scheduler orders systems by their declared access
//! and any explicit ordering; across stages the order is the order the stages were pushed
//! ([`system_schedule`]). Everything runs on the calling thread. A system whose resource
//! parameters can't be fetched is skipped, then tried once more after its layer's commands are
//! applied (so it can see a resource those commands inserted). Run conditions ([`condition`]) are
//! the explicit way to gate a system.
//!
//! # Commands
//!
//! [`Commands`](commands::Commands) queues structural changes (spawn, despawn, insert, remove,
//! resource changes) from inside a system. The queue is applied to the world after each
//! scheduler layer, not immediately.
//!
//! # Change detection
//!
//! Every component and resource has an added tick and a changed tick ([`change_detection`]). Each
//! system run gets its own tick, and "changed" means changed since that system last ran.
//! [`Changed`](query::Changed) filters on it, and
//! [`ChangeDetection`](change_detection::ChangeDetection) exposes it on
//! [`Res`](component::Res), [`ResMut`](component::ResMut) and the references a query yields.
//! Removals are tracked separately with [`RemovedComponents`](removal::RemovedComponents).
//!
//! # Examples
//!
//! ```
//! use weaver_ecs::prelude::*;
//!
//! struct Position(f32);
//! struct Velocity(f32);
//!
//! #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
//! struct Update;
//!
//! fn movement(mut query: Query<(&mut Position, &Velocity)>) {
//!     for (mut position, velocity) in query.iter() {
//!         position.0 += velocity.0;
//!     }
//! }
//!
//! let mut world = World::new();
//! let entity = world.spawn((Position(0.0), Velocity(2.0)));
//!
//! world.push_update_stage(Update);
//! world.add_system(movement, Update);
//! world.initialize_systems();
//! world.update().unwrap();
//!
//! let mut query = world.query::<&Position>();
//! assert_eq!(query.get(entity).unwrap().0, 2.0);
//! ```

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::unused_async)]

pub mod bundle;
pub mod change_detection;
pub mod commands;
pub mod component;
mod component_vec;
pub mod condition;
pub mod entity;
pub mod loan;
pub mod query;
pub mod removal;
pub mod storage;
pub mod system;
pub mod system_schedule;
pub mod world;

pub use weaver_ecs_macros::*;

/// The items most code using the ECS needs: world, queries, systems, commands, bundles and the
/// [`SystemStage`] derive.
pub mod prelude {
    pub use crate::SystemStage;
    pub use crate::bundle::*;
    pub use crate::commands::*;
    pub use crate::component::*;
    pub use crate::condition::{
        any_with_component, not, resource_added, resource_changed, resource_exists,
    };
    pub use crate::entity::*;
    pub use crate::loan::*;
    pub use crate::query::*;
    pub use crate::removal::RemovedComponents;
    pub use crate::storage::*;
    pub use crate::system::*;
    pub use crate::system_schedule::*;
    pub use crate::world::*;
}
