use weaver::{
    weaver_asset::{Assets, Handle},
    weaver_core::{color::Color, transform::Transform},
    weaver_ecs::reflect::{
        registry::{Struct, TypeInfo, TypeRegistry},
        Reflect,
    },
    weaver_pbr::{light::PointLight, material::Material},
};
use weaver_egui::prelude::*;

pub trait InspectUi {
    fn inspect_ui(&mut self, type_registry: &TypeRegistry, assets: &Assets, ui: &mut egui::Ui);
}

impl InspectUi for dyn Reflect {
    fn inspect_ui(&mut self, type_registry: &TypeRegistry, assets: &Assets, ui: &mut egui::Ui) {
        macro_rules! try_downcast {
            ($($t:ty),+ $(,)?) => {
                $(
                    if let Some(value) = self.downcast_mut::<$t>() {
                        return value.inspect_ui(type_registry, assets, ui);
                    }
                )+
            };
        }
        try_downcast!(Color, Handle<Material>, Transform, PointLight);

        if let Some(struct_ref) = self.as_struct_mut() {
            struct_ref.inspect_ui(type_registry, assets, ui);
        } else {
            ui.label(self.reflect_type_name());
        }
    }
}

impl InspectUi for dyn Struct {
    fn inspect_ui(&mut self, type_registry: &TypeRegistry, assets: &Assets, ui: &mut egui::Ui) {
        let Some(type_registration) = type_registry.get_type_info_by_id(self.type_id()) else {
            return;
        };
        let TypeInfo::Struct(struct_info) = type_registration.type_info else {
            return;
        };
        if struct_info.fields.is_empty() {
            ui.label(self.reflect_type_name());
            return;
        }
        ui.collapsing(self.reflect_type_name(), |ui| {
            for field_name in struct_info.field_names.iter() {
                let field = self.field_mut(field_name).unwrap();
                ui.vertical(|ui| {
                    ui.label(field_name.to_string());
                    field.inspect_ui(type_registry, assets, ui);
                });
            }
        });
    }
}

impl InspectUi for Color {
    fn inspect_ui(&mut self, _type_registry: &TypeRegistry, _assets: &Assets, ui: &mut egui::Ui) {
        ui.collapsing(self.reflect_type_name(), |ui| {
            let mut color = self.to_rgb();
            ui.color_edit_button_rgb(&mut color);
            *self = Color::from_rgb(color);
        });
    }
}

impl InspectUi for Handle<Material> {
    fn inspect_ui(&mut self, type_registry: &TypeRegistry, assets: &Assets, ui: &mut egui::Ui) {
        let mut material = assets.get_mut(*self).unwrap();
        material.inspect_ui(type_registry, assets, ui);
    }
}

impl InspectUi for Material {
    fn inspect_ui(&mut self, type_registry: &TypeRegistry, assets: &Assets, ui: &mut egui::Ui) {
        ui.collapsing(self.reflect_type_name(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Diffuse");
                self.diffuse.inspect_ui(type_registry, assets, ui);
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
        });
    }
}

impl InspectUi for Transform {
    fn inspect_ui(&mut self, _type_registry: &TypeRegistry, _assets: &Assets, ui: &mut egui::Ui) {
        ui.collapsing(self.reflect_type_name(), |ui| {
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
        });
    }
}

impl InspectUi for PointLight {
    fn inspect_ui(&mut self, type_registry: &TypeRegistry, assets: &Assets, ui: &mut egui::Ui) {
        ui.collapsing(self.reflect_type_name(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Color");
                self.color.inspect_ui(type_registry, assets, ui);
            });
            ui.horizontal(|ui| {
                ui.label("Intensity");
                ui.add(
                    egui::DragValue::new(&mut self.intensity)
                        .fixed_decimals(2)
                        .speed(0.1)
                        .clamp_range(0.0..=f32::INFINITY),
                );
            });
            ui.horizontal(|ui| {
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
