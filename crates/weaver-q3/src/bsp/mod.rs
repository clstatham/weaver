use loader::{Bsp, BspLoader};
use weaver_app::{App, plugin::Plugin};
use weaver_asset::{AssetApp, DirectLoader, PathAndFilesystem};
use weaver_core::mesh::Mesh;
use weaver_renderer::RenderApp;
use weaver_util::prelude::*;

use crate::shader::loader::LoadedShader;

pub mod generator;
pub mod loader;
pub mod parser;
pub mod render;

pub struct BspPlugin;

impl Plugin for BspPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_asset::<Bsp>();
        app.add_asset::<LoadedShader>();

        app.add_asset_loader::<BspLoader, PathAndFilesystem>();

        app.add_asset_load_dependency::<BspLoader, PathAndFilesystem, DirectLoader<Mesh>, Mesh>();
        app.add_asset_load_dependency::<BspLoader, PathAndFilesystem, DirectLoader<LoadedShader>, LoadedShader>();

        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(render::BspRenderPlugin)?;
        Ok(())
    }
}
