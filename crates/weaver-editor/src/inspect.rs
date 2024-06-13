use weaver::{
    weaver_core::color::Color,
    weaver_ecs::reflect::{
        registry::{Struct, TypeInfo, TypeRegistry},
        Reflect,
    },
    weaver_pbr::material::Material,
};
use weaver_egui::prelude::*;

pub trait InspectUi {
    fn inspect_ui(&mut self, registry: &TypeRegistry, ui: &mut egui::Ui);
}

impl InspectUi for Color {
    fn inspect_ui(&mut self, _registry: &TypeRegistry, ui: &mut egui::Ui) {
        let mut color = self.to_rgb();
        ui.color_edit_button_rgb(&mut color);
        *self = Color::from_rgb(color);
    }
}

impl InspectUi for Material {
    fn inspect_ui(&mut self, registry: &TypeRegistry, ui: &mut egui::Ui) {
        ui.label("Material");
        ui.horizontal(|ui| {
            ui.label("Diffuse");
            self.diffuse.inspect_ui(registry, ui);
        });
        ui.horizontal(|ui| {
            ui.label("Metallic");
            ui.add(
                egui::DragValue::new(&mut self.metallic)
                    .fixed_decimals(2)
                    .speed(0.01)
                    .clamp_range(0.0..=1.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Roughness");
            ui.add(
                egui::DragValue::new(&mut self.roughness)
                    .fixed_decimals(2)
                    .speed(0.01)
                    .clamp_range(0.0..=1.0),
            );
        });
    }
}
