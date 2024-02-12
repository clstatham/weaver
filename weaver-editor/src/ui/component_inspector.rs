use std::ops::RangeInclusive;

use fabricate::component::{runtime::Has, ValueRef};
use weaver::prelude::*;

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

pub fn inspect_value(value: ValueRef, ui: &mut egui::Ui) {
    match value.typ {
        t if t == bool::type_id() => {
            let mut v = *value.value.as_any_mut().downcast_ref::<bool>().unwrap();
            ui.add(egui::Checkbox::new(&mut v, value.name));
            *value.value.as_any_mut().downcast_mut::<bool>().unwrap() = v;
        }
        t if t == i32::type_id() => {
            let mut v = *value.value.as_any_mut().downcast_ref::<i32>().unwrap();
            ui.horizontal(|ui| {
                ui.label(value.name);
                drag(ui, &mut v, None);
            });
            *value.value.as_any_mut().downcast_mut::<i32>().unwrap() = v;
        }
        t if t == i64::type_id() => {
            let mut v = *value.value.as_any_mut().downcast_ref::<i64>().unwrap();
            ui.horizontal(|ui| {
                ui.label(value.name);
                drag(ui, &mut v, None);
            });
            *value.value.as_any_mut().downcast_mut::<i64>().unwrap() = v;
        }
        t if t == f32::type_id() => {
            let mut v = *value.value.as_any_mut().downcast_ref::<f32>().unwrap();
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
            let v = *value.value.as_any_mut().downcast_ref::<Color>().unwrap();
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
            let mut v = *value.value.as_any_mut().downcast_ref::<Vec3>().unwrap();
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
            let mut v = *value.value.as_any_mut().downcast_ref::<Vec4>().unwrap();
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

pub fn component_inspector_ui(
    world: &World,
    commands: &mut Commands,
    state: &mut EditorState,
    ui: &mut egui::Ui,
) {
    if let Some(selected_component) = state.selected_component {
        let mut component = selected_component
            .type_id()
            .and_then(|type_id| world.storage().find_mut(type_id, selected_component));
        if let Some(ref mut component) = component {
            if let Some(component) = component.as_dynamic_mut() {
                for value in component.data_mut().data_mut().inspect() {
                    inspect_value(value, ui);
                }
            } else if let Some(component_ptr) = component.as_pointer_mut() {
                let target = component_ptr.target_entity();
                let has = world.get_relatives_id(target, Has::type_id().id()).unwrap();
                for (has, value) in has {
                    let has = has.as_dynamic().unwrap();
                    let has = has.as_ref::<Has>().unwrap();
                    let name = has.name.to_owned();

                    value
                        .with_component_id_mut(
                            commands.world(),
                            value.type_id().unwrap(),
                            |mut value| {
                                if let Some(value) = value.as_dynamic_mut() {
                                    let value = ValueRef {
                                        name: &name,
                                        typ: value.type_id(),
                                        value: value.data_mut().data_mut(),
                                    };

                                    inspect_value(value, ui);
                                } else {
                                    todo!("Inspecting non-dynamic component")
                                }
                            },
                        )
                        .unwrap();
                }
            }
        } else {
            todo!("Inspecting non-dynamic component")
        }
    }
}
