//! System access declarations, which the scheduler relies on to keep systems apart.
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-ecs -- --ignored`.

use std::any::TypeId;

use weaver_ecs::prelude::*;

struct A;
struct R;

fn reads_a() -> SystemAccess {
    SystemAccess {
        components_read: [TypeId::of::<A>()].into_iter().collect(),
        ..Default::default()
    }
}

fn writes_a() -> SystemAccess {
    SystemAccess {
        components_written: [TypeId::of::<A>()].into_iter().collect(),
        ..Default::default()
    }
}

/// A reader and a writer of the same component must not run at the same time.
#[test]
#[ignore = "known issue: is_compatible only treats write/write overlap as a conflict"]
fn read_write_overlap_is_a_conflict() {
    assert!(!writes_a().is_compatible(&reads_a()));
    assert!(!reads_a().is_compatible(&writes_a()));
}

/// One system asking for overlapping read and write access must be rejected when its access is
/// computed, rather than deadlocking (queries) or panicking (resources) when it runs.
#[test]
#[ignore = "known issue: overlapping read/write params in one system pass the access check"]
fn overlapping_params_in_one_system_are_rejected() {
    let queries = std::panic::catch_unwind(<(Query<&mut A>, Query<&A>) as SystemParam>::access);
    assert!(queries.is_err(), "(Query<&mut A>, Query<&A>) was accepted");

    let resources = std::panic::catch_unwind(<(Res<R>, ResMut<R>) as SystemParam>::access);
    assert!(resources.is_err(), "(Res<R>, ResMut<R>) was accepted");
}

/// `Changed<&mut T>` takes a write lock on T's columns, so it must declare a write.
#[test]
#[ignore = "known issue: Changed<&mut T> declares a read but locks the column for writing"]
fn changed_mut_declares_write_access() {
    let access = <Query<Changed<&mut A>> as SystemParam>::access();
    assert!(access.components_written.contains(&TypeId::of::<A>()));
}
