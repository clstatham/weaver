use weaver::{
    weaver_core::{color::Color, transform::Transform},
    weaver_pbr::material::Material,
};
use weaver_egui::prelude::*;

pub trait InspectUi {
    fn inspect_ui(&mut self, ui: &mut egui::Ui);
}

impl InspectUi for Color {
    fn inspect_ui(&mut self, ui: &mut egui::Ui) {
        let mut color = self.to_rgb();
        ui.color_edit_button_rgb(&mut color);
        *self = Color::from_rgb(color);
    }
}

impl InspectUi for Material {
    fn inspect_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Diffuse");
            self.diffuse.inspect_ui(ui);
        });
        ui.horizontal(|ui| {
            ui.label("Metallic");
            ui.add(
                egui::DragValue::new(&mut self.metallic)
                    .fixed_decimals(2)
                    .speed(0.01)
                    .clamp_range(0.0..=f32::INFINITY),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Roughness");
            ui.add(
                egui::DragValue::new(&mut self.roughness)
                    .fixed_decimals(2)
                    .speed(0.01)
                    .clamp_range(0.0..=f32::INFINITY),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Texture Scale");
            ui.add(
                egui::DragValue::new(&mut self.texture_scale)
                    .fixed_decimals(2)
                    .speed(0.1)
                    .clamp_range(0.0..=f32::INFINITY),
            );
        });
    }
}

impl InspectUi for Transform {
    fn inspect_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Translation");
            ui.add(
                egui::DragValue::new(&mut self.translation.x)
                    .fixed_decimals(2)
                    .speed(0.1),
            );
            ui.add(
                egui::DragValue::new(&mut self.translation.y)
                    .fixed_decimals(2)
                    .speed(0.1),
            );
            ui.add(
                egui::DragValue::new(&mut self.translation.z)
                    .fixed_decimals(2)
                    .speed(0.1),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Rotation");
            ui.add(
                egui::DragValue::new(&mut self.rotation.x)
                    .fixed_decimals(2)
                    .speed(0.1),
            );
            ui.add(
                egui::DragValue::new(&mut self.rotation.y)
                    .fixed_decimals(2)
                    .speed(0.1),
            );
            ui.add(
                egui::DragValue::new(&mut self.rotation.z)
                    .fixed_decimals(2)
                    .speed(0.1),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Scale");
            ui.add(
                egui::DragValue::new(&mut self.scale.x)
                    .fixed_decimals(2)
                    .speed(0.1),
            );
            ui.add(
                egui::DragValue::new(&mut self.scale.y)
                    .fixed_decimals(2)
                    .speed(0.1),
            );
            ui.add(
                egui::DragValue::new(&mut self.scale.z)
                    .fixed_decimals(2)
                    .speed(0.1)
                    .clamp_range(0.0..=f32::INFINITY),
            );
        });
    }
}
