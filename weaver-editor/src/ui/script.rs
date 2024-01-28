use egui::TextEdit;
use weaver::{core::scripts::Scripts, prelude::*};

pub fn script_console_ui(ui: &mut egui::Ui, scripts: &mut Scripts) {
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
                egui::TextEdit::multiline(&mut error.to_string())
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .interactive(false)
                    .text_color(egui::Color32::LIGHT_RED)
                    .show(ui);
            }
        });
    }
}

pub fn script_edit_ui(ui: &mut egui::Ui, scripts: &mut Scripts) {
    for mut script in scripts.script_iter_mut() {
        let mut layouter = |ui: &egui::Ui, string: &str, wrap_width| {
            let mut layout_job = super::syntax_highlighting::highlight(ui.ctx(), string);
            layout_job.wrap.max_width = wrap_width;
            ui.fonts(|f| f.layout_job(layout_job))
        };
        ui.vertical(|ui| {
            if ui.button("Save").clicked() {
                script.save().unwrap();
            }
            ui.separator();
            egui::ScrollArea::both().show(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut script.content)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter),
                );
            });
        });
    }
}
