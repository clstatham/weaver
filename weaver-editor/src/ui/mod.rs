use egui::TextEdit;
use weaver::prelude::{weaver_core::scripts::Scripts, *};

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

#[system(ReloadScripts)]
pub fn reload_scripts(ctx: Res<EguiContext>, scripts: Res<Scripts>) {
    ctx.draw_if_ready(|ctx| {
        egui::Window::new("Reload scripts").show(ctx, |ui| {
            if ui.button("Reload scripts").clicked() {
                scripts.reload();
            }
        });

        for mut script in scripts.script_iter_mut() {
            egui::Window::new(&script.name)
                .max_height(400.0)
                .max_width(800.0)
                .show(ctx, |ui| {
                    if ui.button("Save").clicked() {
                        script.save().unwrap();
                    }
                    TextEdit::multiline(&mut script.content)
                        .desired_width(800.0)
                        .desired_rows(20)
                        .show(ui);
                });
        }
    });
}
