//! Render phases: what each view draws, and how.
//!
//! Every view entity carries one [`RenderPhase<I>`] per phase type. Each frame:
//! 1. [`RenderStage::Queue`]: plugins push phase items (one per thing to draw) into the phases of
//!    the views that should draw them.
//! 2. [`RenderStage::Sort`]: [`sort_phase`] orders each phase by its items' sort keys (and
//!    batching may merge neighbors).
//! 3. [`RenderStage::Render`]: the view driver ([`crate::driver::render_views`]) draws the phases
//!    in a fixed order, handing each run of items to the [`DrawFunction`] they name.
//! 4. [`RenderStage::Cleanup`]: the phases are emptied.
//!
//! So a plugin never orders its drawing against other plugins' systems: it picks a phase, queues
//! items into it and registers a draw function.

use std::{any::TypeId, marker::PhantomData, ops::Range};

use weaver_app::{App, SubApp, plugin::Plugin};
use weaver_asset::AssetId;
use weaver_ecs::{
    commands::Commands,
    entity::Entity,
    query::{Query, With},
    system::{SystemParam, SystemParamItem, SystemState},
    world::World,
};
use weaver_util::prelude::*;

use crate::{RenderStage, tracked_pass::TrackedRenderPass, view::ExtractedView};

/// One thing to draw in a [`RenderPhase`].
pub trait PhaseItem: Send + Sync + 'static {
    type SortKey: Ord;

    /// The entity the item draws (or the view, for items that aren't tied to an entity).
    fn entity(&self) -> Entity;

    fn sort_key(&self) -> Self::SortKey;

    /// The draw function that draws this item.
    fn draw_function(&self) -> DrawFunctionId;
}

/// The items one view draws in one phase this frame.
pub struct RenderPhase<I: PhaseItem> {
    pub items: Vec<I>,
}

impl<I: PhaseItem> Default for RenderPhase<I> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<I: PhaseItem> RenderPhase<I> {
    pub fn add(&mut self, item: I) {
        self.items.push(item);
    }

    pub fn sort(&mut self) {
        self.items.sort_by_key(|item| item.sort_key());
    }

    /// Draws the items, handing each run of items that share a draw function to it in one call.
    pub fn render(
        &self,
        world: &World,
        pass: &mut TrackedRenderPass,
        view: Entity,
        draw_functions: &mut DrawFunctions<I>,
    ) {
        let mut rest = &self.items[..];
        while let Some(first) = rest.first() {
            let id = first.draw_function();
            let run = rest
                .iter()
                .position(|item| item.draw_function() != id)
                .unwrap_or(rest.len());
            draw_functions.draw(id, world, pass, view, &rest[..run]);
            rest = &rest[run..];
        }
    }
}

/// Identifies a [`DrawFunction`] registered in [`DrawFunctions<I>`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DrawFunctionId(usize);

impl DrawFunctionId {
    #[cfg(test)]
    pub(crate) fn for_tests(index: usize) -> Self {
        Self(index)
    }
}

/// Draws a run of phase items into a render pass.
///
/// `Param` is fetched from the render world once per call, like a system's parameters.
pub trait DrawFunction<I: PhaseItem>: Send + Sync + 'static {
    type Param: SystemParam + 'static;

    fn draw(
        param: &mut SystemParamItem<Self::Param>,
        pass: &mut TrackedRenderPass,
        view: Entity,
        items: &[I],
    );
}

trait ErasedDrawFunction<I: PhaseItem>: Send + Sync {
    fn draw(&mut self, world: &World, pass: &mut TrackedRenderPass, view: Entity, items: &[I]);
}

struct DrawFunctionState<I: PhaseItem, D: DrawFunction<I>> {
    state: Option<SystemState<D::Param>>,
    warned: bool,
    _marker: PhantomData<fn(I, D)>,
}

impl<I: PhaseItem, D: DrawFunction<I>> ErasedDrawFunction<I> for DrawFunctionState<I, D> {
    fn draw(&mut self, world: &World, pass: &mut TrackedRenderPass, view: Entity, items: &[I]) {
        let state = self.state.get_or_insert_with(|| SystemState::new(world));
        match state.get(world) {
            Some(mut param) => D::draw(&mut param, pass, view, items),
            None => {
                if !self.warned {
                    self.warned = true;
                    log::warn!(
                        "Skipping draw function {}: its parameters aren't available (a missing resource?). \
                         Items should only be queued once everything they need exists.",
                        std::any::type_name::<D>()
                    );
                }
            }
        }
    }
}

/// The draw functions items of phase `I` can use.
pub struct DrawFunctions<I: PhaseItem> {
    functions: Vec<Box<dyn ErasedDrawFunction<I>>>,
    ids: TypeIdMap<DrawFunctionId>,
}

impl<I: PhaseItem> Default for DrawFunctions<I> {
    fn default() -> Self {
        Self {
            functions: Vec::new(),
            ids: TypeIdMap::default(),
        }
    }
}

impl<I: PhaseItem> DrawFunctions<I> {
    pub fn add<D: DrawFunction<I>>(&mut self) -> DrawFunctionId {
        *self.ids.entry(TypeId::of::<D>()).or_insert_with(|| {
            self.functions.push(Box::new(DrawFunctionState::<I, D> {
                state: None,
                warned: false,
                _marker: PhantomData,
            }));
            DrawFunctionId(self.functions.len() - 1)
        })
    }

    /// The id of draw function `D`. Panics if it wasn't registered with
    /// [`AddDrawFunction::add_draw_function`].
    pub fn id<D: DrawFunction<I>>(&self) -> DrawFunctionId {
        *self.ids.get(&TypeId::of::<D>()).unwrap_or_else(|| {
            panic!(
                "draw function {} isn't registered for phase {}",
                std::any::type_name::<D>(),
                std::any::type_name::<I>()
            )
        })
    }

    fn draw(
        &mut self,
        id: DrawFunctionId,
        world: &World,
        pass: &mut TrackedRenderPass,
        view: Entity,
        items: &[I],
    ) {
        self.functions[id.0].draw(world, pass, view, items);
    }
}

/// Registering draw functions on the render app.
pub trait AddDrawFunction {
    /// Registers draw function `D` for phase `I`. The phase's [`RenderPhasePlugin`] must have been
    /// added first (the renderer adds the built-in phases).
    fn add_draw_function<I: PhaseItem, D: DrawFunction<I>>(&mut self) -> &mut Self;
}

fn add_draw_function<I: PhaseItem, D: DrawFunction<I>>(world: &World) {
    world
        .get_resource_mut::<DrawFunctions<I>>()
        .unwrap_or_else(|| {
            panic!(
                "phase {} isn't set up; add its RenderPhasePlugin first",
                std::any::type_name::<I>()
            )
        })
        .add::<D>();
}

impl AddDrawFunction for SubApp {
    fn add_draw_function<I: PhaseItem, D: DrawFunction<I>>(&mut self) -> &mut Self {
        add_draw_function::<I, D>(self.world());
        self
    }
}

impl AddDrawFunction for App {
    fn add_draw_function<I: PhaseItem, D: DrawFunction<I>>(&mut self) -> &mut Self {
        add_draw_function::<I, D>(self.main_app().world());
        self
    }
}

/// Sets up phase `I`: every view gets a [`RenderPhase<I>`], sorted in [`RenderStage::Sort`] and
/// emptied in [`RenderStage::Cleanup`].
pub struct RenderPhasePlugin<I: PhaseItem>(PhantomData<I>);

impl<I: PhaseItem> Default for RenderPhasePlugin<I> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<I: PhaseItem> Plugin for RenderPhasePlugin<I> {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.init_resource::<DrawFunctions<I>>();
        render_app.add_system(insert_view_phase::<I>, RenderStage::Prepare);
        render_app.add_system(sort_phase::<I>, RenderStage::Sort);
        render_app.add_system(clear_phase::<I>, RenderStage::Cleanup);
        Ok(())
    }
}

#[allow(clippy::type_complexity)]
fn insert_view_phase<I: PhaseItem>(
    commands: Commands,
    mut views: Query<(Entity, With<ExtractedView>, Option<&RenderPhase<I>>)>,
) {
    for (view, _, phase) in views.iter() {
        if phase.is_none() {
            commands.insert_component(view, RenderPhase::<I>::default());
        }
    }
}

pub fn sort_phase<I: PhaseItem>(mut phases: Query<&mut RenderPhase<I>>) {
    for mut phase in phases.iter() {
        phase.sort();
    }
}

fn clear_phase<I: PhaseItem>(mut phases: Query<&mut RenderPhase<I>>) {
    for mut phase in phases.iter() {
        phase.items.clear();
    }
}

/// Drawn first, behind everything: skyboxes and other backgrounds.
pub struct Background3d {
    pub view: Entity,
    pub draw_function: DrawFunctionId,
}

impl PhaseItem for Background3d {
    type SortKey = DrawFunctionId;

    fn entity(&self) -> Entity {
        self.view
    }

    fn sort_key(&self) -> Self::SortKey {
        self.draw_function
    }

    fn draw_function(&self) -> DrawFunctionId {
        self.draw_function
    }
}

/// What opaque mesh items are sorted (and batched) by: items with equal keys draw the same mesh
/// with the same material, so neighbors can be merged into one instanced draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Opaque3dKey {
    pub draw_function: DrawFunctionId,
    pub material: AssetId,
    pub mesh: AssetId,
}

/// An opaque mesh.
pub struct Opaque3d {
    pub key: Opaque3dKey,
    pub entity: Entity,
    /// The instances to draw. Set by batching in [`RenderStage::Sort`]: index `i` in this range
    /// reads the `i`th transform of the view's [`crate::mesh::instances::ViewMeshInstances`].
    pub batch_range: Range<u32>,
}

impl Opaque3d {
    pub fn new(entity: Entity, key: Opaque3dKey) -> Self {
        Self {
            key,
            entity,
            batch_range: 0..0,
        }
    }
}

impl PhaseItem for Opaque3d {
    type SortKey = Opaque3dKey;

    fn entity(&self) -> Entity {
        self.entity
    }

    fn sort_key(&self) -> Self::SortKey {
        self.key
    }

    fn draw_function(&self) -> DrawFunctionId {
        self.key.draw_function
    }
}

/// Drawn last, over the scene: gizmos and other debug overlays.
pub struct Overlay3d {
    pub entity: Entity,
    pub draw_function: DrawFunctionId,
}

impl PhaseItem for Overlay3d {
    type SortKey = DrawFunctionId;

    fn entity(&self) -> Entity {
        self.entity
    }

    fn sort_key(&self) -> Self::SortKey {
        self.draw_function
    }

    fn draw_function(&self) -> DrawFunctionId {
        self.draw_function
    }
}
