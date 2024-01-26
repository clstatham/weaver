use weaver::prelude::*;

use crate::state::EditorState;

pub mod state;
pub mod ui;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    log::info!("weaver editor starting up");

    let app = App::new(1600, 900)?;

    app.add_resource(EditorState::default())?;
    app.add_resource(FpsDisplay::new())?;

    app.add_system_to_stage(ui::UiMain, SystemStage::Update);
    app.add_system_to_stage(ui::ReloadScripts, SystemStage::PreUpdate);

    app.add_script("assets/scripts/editor/main.loom");

    app.run()?;

    Ok(())
}
