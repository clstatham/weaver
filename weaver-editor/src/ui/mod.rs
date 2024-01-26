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
pub fn script_update(ctx: Res<EguiContext>, scripts: Res<Scripts>, input: Res<Input>) {
    ctx.draw_if_ready(|ctx| {
        egui::Window::new("Scripts").show(ctx, |ui| {
            if ui.button("Reload Scripts").clicked() {
                scripts.reload();
            }
            if scripts.has_errors() {
                ui.separator();
                ui.label("!!! Some systems had errors. Check below for details. !!!");
                egui::ScrollArea::both().show(ui, |ui| {
                    for (name, error) in scripts.script_errors() {
                        ui.separator();
                        ui.label(name);
                        TextEdit::multiline(&mut error.to_string())
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .interactive(false)
                            .text_color(egui::Color32::LIGHT_RED)
                            .show(ui);
                    }
                });
            }
        });

        for mut script in scripts.script_iter_mut() {
            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width| {
                let mut layout_job = syntax_highlighting::highlight(ui.ctx(), string);
                layout_job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(layout_job))
            };
            egui::Window::new(&script.name).show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.vertical(|ui| {
                        if ui.button("Save").clicked() {
                            script.save().unwrap();
                        }
                    });
                    ui.separator();
                    egui::ScrollArea::both().show(ui, |ui| {
                        let mut editor = ui.add(
                            TextEdit::multiline(&mut script.content)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(20)
                                .layouter(&mut layouter),
                        );
                        if editor.lost_focus() {
                            script.save().unwrap();
                            input.enable_input();
                        }
                        if editor.has_focus() {
                            input.disable_input();
                        }
                    });
                });
            });
        }
    });
}
