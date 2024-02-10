use std::ops::RangeInclusive;

use weaver::{core::color::Color, prelude::*};

use crate::state::EditorState;

fn drag<N>(ui: &mut egui::Ui, value: &mut N, min_max: Option<RangeInclusive<N>>)
where
    N: egui::emath::Numeric,
{
    let mut v = *value;
    let mut drag = egui::DragValue::new(&mut v);
    if let Some(range) = min_max {
        drag = drag.clamp_range(range);
    }
    ui.add(drag);
    *value = v;
}

pub trait InspectExt: Component {
    fn ui(&mut self, ui: &mut egui::Ui) {
        for value in self.inspect() {
            match value.typ {
                t if t == bool::type_id() => {
                    let mut v = *value.value.as_any_mut().downcast_mut::<bool>().unwrap();
                    ui.add(egui::Checkbox::new(&mut v, value.name));
                    *value.value.as_any_mut().downcast_mut::<bool>().unwrap() = v;
                }
                t if t == i32::type_id() => {
                    let mut v = *value.value.as_any_mut().downcast_mut::<i32>().unwrap();
                    ui.horizontal(|ui| {
                        ui.label(value.name);
                        drag(ui, &mut v, None);
                    });
                    *value.value.as_any_mut().downcast_mut::<i32>().unwrap() = v;
                }
                t if t == f32::type_id() => {
                    let mut v = *value.value.as_any_mut().downcast_mut::<f32>().unwrap();
                    ui.horizontal(|ui| {
                        ui.label(value.name);
                        drag(ui, &mut v, None);
                    });
                    *value.value.as_any_mut().downcast_mut::<f32>().unwrap() = v;
                }
                t if t == String::type_id() => {
                    ui.horizontal(|ui| {
                        let v = value.value.as_any_mut().downcast_mut::<String>().unwrap();
                        ui.label(value.name);
                        ui.add(egui::TextEdit::singleline(v));
                    });
                }
                t if t == Color::type_id() => {
                    let v = *value.value.as_any_mut().downcast_mut::<Color>().unwrap();
                    let (r, g, b, a) = v.rgba_int();
                    let mut v = egui::Color32::from_rgb(r, g, b);
                    ui.horizontal(|ui| {
                        ui.label(value.name);
                        egui::color_picker::color_picker_color32(
                            ui,
                            &mut v,
                            egui::color_picker::Alpha::OnlyBlend,
                        );
                    });

                    let v = Color::from_rgba_int(v.r(), v.g(), v.b(), a);
                    *value.value.as_any_mut().downcast_mut::<Color>().unwrap() = v;
                }
                t if t == Vec3::type_id() => {
                    let mut v = *value.value.as_any_mut().downcast_mut::<Vec3>().unwrap();
                    ui.horizontal(|ui| {
                        ui.label(value.name);
                        ui.vertical(|ui| {
                            ui.label("x");
                            drag(ui, &mut v.x, None);
                        });
                        ui.vertical(|ui| {
                            ui.label("y");
                            drag(ui, &mut v.y, None);
                        });
                        ui.vertical(|ui| {
                            ui.label("z");
                            drag(ui, &mut v.z, None);
                        });
                    });
                    *value.value.as_any_mut().downcast_mut::<Vec3>().unwrap() = v;
                }
                t if t == Vec4::type_id() => {
                    let mut v = *value.value.as_any_mut().downcast_mut::<Vec4>().unwrap();
                    ui.horizontal(|ui| {
                        ui.label(value.name);
                        ui.vertical(|ui| {
                            ui.label("x");
                            drag(ui, &mut v.x, None);
                        });
                        ui.vertical(|ui| {
                            ui.label("y");
                            drag(ui, &mut v.y, None);
                        });
                        ui.vertical(|ui| {
                            ui.label("z");
                            drag(ui, &mut v.z, None);
                        });
                        ui.vertical(|ui| {
                            ui.label("w");
                            drag(ui, &mut v.w, None);
                        });
                    });
                    *value.value.as_any_mut().downcast_mut::<Vec4>().unwrap() = v;
                }
                _ => {}
            }
        }
    }
}

impl<T: Component + ?Sized> InspectExt for T {}

pub fn component_inspector_ui(world: &World, state: &mut EditorState, ui: &mut egui::Ui) {
    if let Some(component) = state.selected_component {
        let mut component = world
            .storage()
            .find_mut(component.type_id().unwrap(), component)
            .unwrap();
        component
            .as_dynamic_mut()
            .unwrap()
            .data_mut()
            .data_mut()
            .ui(ui);
    }
}
