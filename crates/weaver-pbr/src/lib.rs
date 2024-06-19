use light::PointLightPlugin;
use material::MaterialPlugin;
use render::{PbrNode, PbrNodeLabel};
use weaver_app::prelude::*;
use weaver_renderer::{
    clear_color::{ClearColorLabel, ClearColorNode},
    graph::{GraphInputLabel, RenderGraphApp, ViewNodeRunner},
    pipeline::RenderPipelinePlugin,
    RenderApp, RenderLabel,
};
use weaver_util::prelude::*;

pub mod light;
pub mod material;
pub mod render;

pub mod prelude {
    pub use crate::light::*;
    pub use crate::material::*;
    pub use crate::PbrPlugin;
}

#[derive(Clone, Copy, Debug)]
pub struct PbrSubGraph;
impl RenderLabel for PbrSubGraph {}

pub struct PbrPlugin;

impl Plugin for PbrPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(MaterialPlugin)?;
        render_app.add_plugin(PointLightPlugin)?;
        render_app.add_plugin(RenderPipelinePlugin::<PbrNode>::default())?;

        render_app.add_render_sub_graph(PbrSubGraph, vec![]);
        render_app.add_render_sub_graph_node::<ViewNodeRunner<ClearColorNode>>(
            PbrSubGraph,
            ClearColorLabel,
        );
        render_app.add_render_sub_graph_node::<ViewNodeRunner<PbrNode>>(PbrSubGraph, PbrNodeLabel);
        render_app.add_render_sub_graph_edge(PbrSubGraph, GraphInputLabel, ClearColorLabel);
        render_app.add_render_sub_graph_edge(PbrSubGraph, ClearColorLabel, PbrNodeLabel);

        Ok(())
    }
}
