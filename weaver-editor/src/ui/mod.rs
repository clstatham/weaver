use weaver::prelude::*;

use crate::state::EditorState;

#[system(UiMain)]
pub fn ui_main(ctx: Res<EguiContext>, state: Res<EditorState>, fps_display: ResMut<FpsDisplay>) {
    ctx.draw_if_ready(|ctx| {
        fps_display.run_ui(ctx);
        egui::Window::new("Hello world").show(ctx, |ui| {
            ui.label("Hello world!");
            ui.label(format!("Editor state: {:?}", &*state));
        });
    });
}
