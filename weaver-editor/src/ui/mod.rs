use egui::TextEdit;
use weaver::prelude::{weaver_core::scripts::Scripts, *};

use crate::state::EditorState;
pub mod syntax_highlighting;

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

#[system(ScriptUpdate)]
pub fn reload_scripts(ctx: Res<EguiContext>, scripts: Res<Scripts>) {
    ctx.draw_if_ready(|ctx| {
        egui::Window::new("Reload scripts").show(ctx, |ui| {
            if ui.button("Reload scripts").clicked() {
                scripts.reload();
            }
        });

        for mut script in scripts.script_iter_mut() {
            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width| {
                let mut layout_job = syntax_highlighting::highlight(ui.ctx(), &string);
                layout_job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(layout_job))
            };
            egui::Window::new(&script.name)
                .max_height(400.0)
                .max_width(800.0)
                .scroll2([true, true])
                .show(ctx, |ui| {
                    if ui.button("Save").clicked() {
                        script.save().unwrap();
                    }
                    ui.add(
                        TextEdit::multiline(&mut script.content)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(20)
                            .layouter(&mut layouter),
                    );
                });
        }
    });
}
