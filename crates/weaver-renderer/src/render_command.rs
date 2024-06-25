use weaver_app::{App, SubApp};
use weaver_ecs::{
    entity::Entity,
    query::{Query, QueryFetch, QueryFilter},
    system::{SystemParam, SystemParamItem, SystemState},
    world::{World, WorldLock},
};
use weaver_util::{
    error_once,
    lock::{Lock, SharedLock},
    prelude::{bail, Result},
};

use crate::{
    draw_fn::DrawItem,
    prelude::{DrawFn, DrawFunctions},
};

pub trait RenderCommand<T: DrawItem>: 'static + Send + Sync {
    type Param: SystemParam + Send + Sync + 'static;
    type ViewQueryFetch: QueryFetch;
    type ViewQueryFilter: QueryFilter;
    type ItemQueryFetch: QueryFetch;
    type ItemQueryFilter: QueryFilter;

    fn render<'w>(
        item: T,
        view_query: <Self::ViewQueryFetch as QueryFetch>::Item,
        item_query: Option<<Self::ItemQueryFetch as QueryFetch>::Item>,
        param: SystemParamItem<'w, '_, Self::Param>,
        render_pass: &mut wgpu::RenderPass<'w>,
    ) -> Result<()>;
}

pub struct RenderCommandState<T: DrawItem, C: RenderCommand<T>> {
    view_query: Query<C::ViewQueryFetch, C::ViewQueryFilter>,
    item_query: Query<C::ItemQueryFetch, C::ItemQueryFilter>,
    state: Option<SystemState<C::Param>>,
}

impl<T: DrawItem, C: RenderCommand<T>> RenderCommandState<T, C> {
    pub fn new(render_world: &World) -> Self {
        Self {
            view_query: render_world.query_filtered(),
            item_query: render_world.query_filtered(),
            state: None,
        }
    }
}

impl<T: DrawItem, C: RenderCommand<T>> DrawFn<T> for RenderCommandState<T, C> {
    fn prepare(&mut self, render_world: &WorldLock) -> Result<()> {
        self.view_query = render_world.query_filtered();
        self.item_query = render_world.query_filtered();
        self.state = Some(SystemState::new(render_world));
        Ok(())
    }

    fn draw<'w>(
        &mut self,
        render_world: &'w WorldLock,
        render_pass: &mut wgpu::RenderPass<'w>,
        view_entity: Entity,
        item: T,
    ) -> Result<()> {
        let Some(view_query) = self.view_query.get(view_entity) else {
            error_once!(
                "View query not found for RenderCommand {:?}",
                std::any::type_name::<C>()
            );
            bail!("View query not found for RenderCommand");
        };
        let item_query = self.item_query.get(item.entity());
        let state = self.state.as_mut().unwrap();
        let param = state.get(render_world);

        C::render(item, view_query, item_query, param, render_pass)
    }
}

pub trait AddRenderCommand {
    fn add_render_command<T: DrawItem, C: RenderCommand<T>>(&mut self) -> &mut Self;
}

impl AddRenderCommand for SubApp {
    fn add_render_command<T: DrawItem, C: RenderCommand<T>>(&mut self) -> &mut Self {
        let draw_fn = RenderCommandState::<T, C>::new(&self.read_world());
        if let Some(draw_fns) = self.get_resource::<DrawFunctions<T>>() {
            draw_fns.write().add(draw_fn);
        } else {
            let draw_fns = DrawFunctions::<T>::new();
            draw_fns.write().add(draw_fn);
            self.insert_resource(draw_fns);
        }

        self
    }
}

impl AddRenderCommand for App {
    fn add_render_command<T: DrawItem, C: RenderCommand<T>>(&mut self) -> &mut Self {
        self.main_app_mut().add_render_command::<T, C>();
        self
    }
}
