use weaver::{
    weaver_asset::{Assets, Handle},
    weaver_core::{color::Color, transform::Transform},
    weaver_ecs::{component::Component, reflect::Reflect, world::World},
    weaver_pbr::{light::PointLight, material::Material},
};
use weaver_egui::prelude::*;

pub trait InspectUi {
    fn inspect_ui(&mut self, world: &mut World, ui: &mut egui::Ui);
}

impl InspectUi for dyn Component {
    fn inspect_ui(&mut self, world: &mut World, ui: &mut egui::Ui) {
        macro_rules! try_downcast {
            ($($t:ty),*) => {
                $(
                    if let Some(value) = self.downcast_mut::<$t>() {
                        value.inspect_ui(world, ui);
                        return;
                    }
                )*
            };
        }
        try_downcast!(Color, Handle<Material>, Transform, PointLight);
    }
}

impl InspectUi for Color {
    fn inspect_ui(&mut self, __world: &mut World, ui: &mut egui::Ui) {
        let mut color = self.to_rgb();
        ui.color_edit_button_rgb(&mut color);
        *self = Color::from_rgb(color);
    }
}

impl InspectUi for Handle<Material> {
    fn inspect_ui(&mut self, world: &mut World, ui: &mut egui::Ui) {
        let assets = world.get_resource::<Assets<Material>>().unwrap();
        let mut material = assets.get_mut(*self).unwrap();

        ui.collapsing(material.reflect_type_name(), |ui| {
            ui.horizontal_top(|ui| {
                ui.label("Diffuse");
                let mut color = material.diffuse.to_rgb();
                ui.color_edit_button_rgb(&mut color);
                material.diffuse = Color::from_rgb(color);
            });
            ui.horizontal_top(|ui| {
                ui.label("Metallic");
                ui.add(
                    egui::DragValue::new(&mut material.metallic)
                        .fixed_decimals(2)
                        .speed(0.01)
                        .clamp_range(0.0..=1.0),
                );
            });
            ui.horizontal_top(|ui| {
                ui.label("Roughness");
                ui.add(
                    egui::DragValue::new(&mut material.roughness)
                        .fixed_decimals(2)
                        .speed(0.01)
                        .clamp_range(0.0..=1.0),
                );
            });
            ui.horizontal_top(|ui| {
                ui.label("Texture Scale");
                ui.add(
                    egui::DragValue::new(&mut material.texture_scale)
                        .fixed_decimals(2)
                        .speed(0.1)
                        .clamp_range(0.0..=f32::INFINITY),
                );
            });
        });
    }
}

impl InspectUi for Transform {
    fn inspect_ui(&mut self, __world: &mut World, ui: &mut egui::Ui) {
        ui.collapsing(self.reflect_type_name(), |ui| {
            ui.horizontal_top(|ui| {
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
            ui.horizontal_top(|ui| {
                ui.label("Scale");
                ui.add(
                    egui::DragValue::new(&mut self.scale.x)
                        .fixed_decimals(2)
                        .speed(0.1)
                        .clamp_range(0.0..=f32::INFINITY),
                );
                ui.add(
                    egui::DragValue::new(&mut self.scale.y)
                        .fixed_decimals(2)
                        .speed(0.1)
                        .clamp_range(0.0..=f32::INFINITY),
                );
                ui.add(
                    egui::DragValue::new(&mut self.scale.z)
                        .fixed_decimals(2)
                        .speed(0.1)
                        .clamp_range(0.0..=f32::INFINITY),
                );
            });
        });
    }
}

impl InspectUi for PointLight {
    fn inspect_ui(&mut self, __world: &mut World, ui: &mut egui::Ui) {
        ui.collapsing(self.reflect_type_name(), |ui| {
            ui.horizontal_top(|ui| {
                ui.label("Enabled");
                ui.checkbox(&mut self.enabled, "");
            });
            ui.horizontal_top(|ui| {
                ui.label("Color");
                let mut color = self.color.to_rgb();
                ui.color_edit_button_rgb(&mut color);
                self.color = Color::from_rgb(color);
            });
            ui.horizontal_top(|ui| {
                ui.label("Intensity");
                ui.add(
                    egui::DragValue::new(&mut self.intensity)
                        .fixed_decimals(2)
                        .speed(0.1)
                        .clamp_range(0.0..=f32::INFINITY),
                );
            });
            ui.horizontal_top(|ui| {
                ui.label("Radius");
                ui.add(
                    egui::DragValue::new(&mut self.radius)
                        .fixed_decimals(2)
                        .speed(0.1)
                        .clamp_range(0.0..=f32::INFINITY),
                );
            });
        });
    }
}
