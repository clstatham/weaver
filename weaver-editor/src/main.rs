use weaver::prelude::*;

use crate::state::EditorState;

pub mod state;
pub mod ui;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    log::info!("weaver editor starting up");

    let app = App::new(1600, 900)?;

    app.insert_resource(EditorState::default())?;
    app.insert_resource(FpsDisplay::new())?;

    app.add_system_to_stage(ui::UiMain, SystemStage::Update);

    app.run()?;

    Ok(())
}
