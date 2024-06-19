use weaver_app::{App, SubApp};
use weaver_ecs::{
    component::Res,
    entity::Entity,
    query::{Query, QueryFetch, QueryFilter},
    system::SystemParam,
    world::World,
};
use weaver_util::prelude::Result;

use crate::{
    draw_fn::{DrawItem, RenderPipelinedDrawItem},
    pipeline::RenderPipelineCache,
    prelude::{DrawFn, DrawFunctions},
};

pub trait RenderCommand<T: DrawItem>: 'static + Send + Sync {
    type Param: SystemParam + Send + Sync;
    type ViewQueryFetch: QueryFetch;
    type ViewQueryFilter: QueryFilter;
    type ItemQueryFetch: QueryFetch;
    type ItemQueryFilter: QueryFilter;

    fn render<'w>(
        item: &T,
        view_query: &<Self::ViewQueryFetch as QueryFetch>::Fetch,
        item_query: Option<&<Self::ItemQueryFetch as QueryFetch>::Fetch>,
        param: &'w Self::Param,
        pass: &mut wgpu::RenderPass<'w>,
    ) -> Result<()>;
}

macro_rules! impl_render_command_tuple {
    ($(
        ($ty:ident, $param:ident, $view:ident, $item:ident)
    ),*) => {
        impl<T: DrawItem, $($ty: RenderCommand<T>),*> RenderCommand<T> for ($($ty,)*) {
            type Param = ($($ty::Param,)*);
            type ViewQueryFetch = ($($ty::ViewQueryFetch,)*);
            type ViewQueryFilter = ($($ty::ViewQueryFilter,)*);
            type ItemQueryFetch = ($($ty::ItemQueryFetch,)*);
            type ItemQueryFilter = ($($ty::ItemQueryFilter,)*);

            fn render<'w>(
                item: &T,
                ($($view,)*): &<Self::ViewQueryFetch as QueryFetch>::Fetch,
                item_query: Option<&<Self::ItemQueryFetch as QueryFetch>::Fetch>,
                ($($param,)*): &'w Self::Param,
                pass: &mut wgpu::RenderPass<'w>,
            ) -> Result<()> {
                match item_query {
                    Some(($($item,)*)) => {
                        $(
                            $ty::render(item, $view, Some($item), $param, pass)?;
                        )*
                    }
                    None => {
                        $(
                            $ty::render(item, $view, None, $param, pass)?;
                        )*
                    }
                }

                Ok(())
            }
        }
    };
}

impl_render_command_tuple!((A, a_param, a_view, a_item), (B, b_param, b_view, b_item));
impl_render_command_tuple!(
    (A, a_param, a_view, a_item),
    (B, b_param, b_view, b_item),
    (C, c_param, c_view, c_item)
);
impl_render_command_tuple!(
    (A, a_param, a_view, a_item),
    (B, b_param, b_view, b_item),
    (C, c_param, c_view, c_item),
    (D, d_param, d_view, d_item)
);
impl_render_command_tuple!(
    (A, a_param, a_view, a_item),
    (B, b_param, b_view, b_item),
    (C, c_param, c_view, c_item),
    (D, d_param, d_view, d_item),
    (E, e_param, e_view, e_item)
);
impl_render_command_tuple!(
    (A, a_param, a_view, a_item),
    (B, b_param, b_view, b_item),
    (C, c_param, c_view, c_item),
    (D, d_param, d_view, d_item),
    (E, e_param, e_view, e_item),
    (F, f_param, f_view, f_item)
);
impl_render_command_tuple!(
    (A, a_param, a_view, a_item),
    (B, b_param, b_view, b_item),
    (C, c_param, c_view, c_item),
    (D, d_param, d_view, d_item),
    (E, e_param, e_view, e_item),
    (F, f_param, f_view, f_item),
    (G, g_param, g_view, g_item)
);
impl_render_command_tuple!(
    (A, a_param, a_view, a_item),
    (B, b_param, b_view, b_item),
    (C, c_param, c_view, c_item),
    (D, d_param, d_view, d_item),
    (E, e_param, e_view, e_item),
    (F, f_param, f_view, f_item),
    (G, g_param, g_view, g_item),
    (H, h_param, h_view, h_item)
);

pub struct RenderCommandState<T: DrawItem, C: RenderCommand<T>> {
    param: C::Param,
    view_query: Query<C::ViewQueryFetch, C::ViewQueryFilter>,
    item_query: Query<C::ItemQueryFetch, C::ItemQueryFilter>,
}

impl<T: DrawItem, C: RenderCommand<T>> RenderCommandState<T, C> {
    pub fn new(render_world: &mut World) -> Self {
        Self {
            param: C::Param::fetch(render_world).unwrap(),
            view_query: render_world.query_filtered(),
            item_query: render_world.query_filtered(),
        }
    }
}

impl<T: DrawItem, C: RenderCommand<T>> DrawFn<T> for RenderCommandState<T, C> {
    fn prepare(&mut self, render_world: &World) -> Result<()> {
        self.param = C::Param::fetch(render_world).unwrap();
        self.view_query = render_world.query_filtered();
        self.item_query = render_world.query_filtered();
        Ok(())
    }

    fn draw<'w>(
        &'w mut self,
        _render_world: &World,
        render_pass: &mut wgpu::RenderPass<'w>,
        view_entity: Entity,
        item: &T,
    ) -> Result<()> {
        let view_query = self.view_query.get(view_entity).unwrap();
        let item_query = self.item_query.get(item.entity());

        <C as RenderCommand<T>>::render(
            item,
            &view_query,
            item_query.as_ref(),
            &self.param,
            render_pass,
        )
    }
}

pub trait AddRenderCommand {
    fn add_render_command<T: DrawItem, C: RenderCommand<T>>(&mut self) -> &mut Self;
}

impl AddRenderCommand for SubApp {
    fn add_render_command<T: DrawItem, C: RenderCommand<T>>(&mut self) -> &mut Self {
        let draw_fn = RenderCommandState::<T, C>::new(self.world_mut());
        let draw_fns = self
            .world_mut()
            .get_resource_mut::<DrawFunctions<T>>()
            .unwrap();
        draw_fns.write().add(draw_fn);
        self
    }
}

impl AddRenderCommand for App {
    fn add_render_command<T: DrawItem, C: RenderCommand<T>>(&mut self) -> &mut Self {
        self.main_app_mut().add_render_command::<T, C>();
        self
    }
}

pub struct SetRenderPipeline;

impl<T: RenderPipelinedDrawItem> RenderCommand<T> for SetRenderPipeline {
    type Param = Res<RenderPipelineCache>;
    type ViewQueryFetch = ();
    type ViewQueryFilter = ();
    type ItemQueryFetch = ();
    type ItemQueryFilter = ();

    fn render<'w>(
        _item: &T,
        _view_query: &(),
        _item_query: Option<&()>,
        param: &'w Self::Param,
        pass: &mut wgpu::RenderPass<'w>,
    ) -> Result<()> {
        let pipeline = param.get_pipeline::<T>().unwrap();
        pass.set_pipeline(pipeline);
        Ok(())
    }
}
