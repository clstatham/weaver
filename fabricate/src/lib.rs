#![doc = include_str!("../README.md")]

pub mod bundle;
pub mod commands;
#[macro_use]
pub mod component;
pub mod graph;
pub mod lock;
pub mod query;
pub mod registry;
pub mod relationship;
pub mod script;
pub mod storage;
pub mod system;
pub mod world;

pub mod prelude {
    pub use crate::{
        bundle::Bundle,
        component::Component,
        graph::{Edge, Graph},
        lock::{MapRead, MapWrite, Read, ReadWrite, SharedLock, Write},
        query::Query,
        registry::{global_registry, Entity, Id, Registry, RegistryHandle},
        script::Script,
        script_vtable,
        storage::{Data, DynamicData, DynamicMut, DynamicRef, Mut, Ref, Storage},
        system::{DynamicSystem, System, SystemGraph, SystemStage},
        world::{get_world, LockedWorldHandle, World},
    };
    pub use anyhow::{anyhow, bail, ensure, Result};
    pub use fabricate_macro::Component;
}
