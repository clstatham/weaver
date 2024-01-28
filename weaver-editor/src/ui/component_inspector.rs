use weaver::prelude::*;

use crate::state::{EditorState, UpdateComponent};

pub fn component_inspector_ui(
    world: &World,
    ui: &mut egui::Ui,
    state: &mut EditorState,
    commands: &mut Commands,
) {
    if let Some(entity) = state.selected_entity() {
        if let Some(component) = state.selected_component() {
            let components = world.components.entity_components_iter(entity.id());
            if components.is_none() {
                return;
            }
            let component = components.unwrap().find(|c| c.type_id() == component);
            if component.is_none() {
                return;
            }
            let component = component.unwrap();
            ui.label(component.name());
            if let Some(fields) = component.fields() {
                let mut any_clicked = false;
                let mut any_released = false;
                let mut any_changed = false;
                for field in fields.iter() {
                    if field.type_id() == world.registry().get_static::<()>() {
                        continue;
                    }
                    let field_name = field.name();
                    ui.horizontal(|ui| {
                        ui.label(format!("{}:", field_name));
                        if let Some(mut value) = field.get_as_mut::<f32>() {
                            ui.horizontal(|ui| {
                                let res = ui.add(egui::DragValue::new(&mut *value));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                            });
                        } else if let Some(mut value) = field.get_as_mut::<i64>() {
                            ui.horizontal(|ui| {
                                let res = ui.add(egui::DragValue::new(&mut *value));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                            });
                        } else if let Some(mut value) = field.get_as_mut::<u64>() {
                            ui.horizontal(|ui| {
                                let res = ui.add(egui::DragValue::new(&mut *value));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                            });
                        } else if let Some(mut value) = field.get_as_mut::<bool>() {
                            let res = ui.add(egui::Checkbox::new(&mut value, field_name));
                            any_clicked |= res.clicked();
                            any_changed |= res.changed();
                        } else if let Some(mut value) = field.get_as_mut::<String>() {
                            let res = ui.add(egui::TextEdit::singleline(&mut *value));
                            any_clicked |= res.clicked();
                            any_changed |= res.changed();
                        } else if let Some(mut value) = field.get_as_mut::<Vec2>() {
                            ui.horizontal(|ui| {
                                ui.label("x");
                                let res = ui.add(egui::DragValue::new(&mut value.x));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("y");
                                let res = ui.add(egui::DragValue::new(&mut value.y));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                            });
                        } else if let Some(mut value) = field.get_as_mut::<Vec3>() {
                            ui.horizontal(|ui| {
                                ui.label("x");
                                let res = ui.add(egui::DragValue::new(&mut value.x));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("y");
                                let res = ui.add(egui::DragValue::new(&mut value.y));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("z");
                                let res = ui.add(egui::DragValue::new(&mut value.z));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                            });

                            // TODO: This is a hack to prevent the scale from going negative or zero.
                            //       Update this when we have a proper way to constrain values.
                            if field_name == "scale" {
                                if value.x <= 0.0 {
                                    value.x = 0.001;
                                }
                                if value.y <= 0.0 {
                                    value.y = 0.001;
                                }
                                if value.z <= 0.0 {
                                    value.z = 0.001;
                                }
                            }
                        } else if let Some(mut value) = field.get_as_mut::<Vec4>() {
                            ui.horizontal(|ui| {
                                ui.label("x");
                                let res = ui.add(egui::DragValue::new(&mut value.x));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("y");
                                let res = ui.add(egui::DragValue::new(&mut value.y));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("z");
                                let res = ui.add(egui::DragValue::new(&mut value.z));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("w");
                                let res = ui.add(egui::DragValue::new(&mut value.w));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                            });
                        } else if let Some(mut value) = field.get_as_mut::<Quat>() {
                            ui.horizontal(|ui| {
                                ui.label("x");
                                let res = ui.add(egui::DragValue::new(&mut value.x));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("y");
                                let res = ui.add(egui::DragValue::new(&mut value.y));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("z");
                                let res = ui.add(egui::DragValue::new(&mut value.z));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("w");
                                let res = ui.add(egui::DragValue::new(&mut value.w));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                            });
                        } else if let Some(mut value) = field.get_as_mut::<Color>() {
                            ui.horizontal(|ui| {
                                ui.label("r");
                                let res = ui.add(egui::DragValue::new(&mut value.r));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("g");
                                let res = ui.add(egui::DragValue::new(&mut value.g));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                                ui.label("b");
                                let res = ui.add(egui::DragValue::new(&mut value.b));
                                any_clicked |= res.drag_started();
                                any_changed |= res.changed();
                                any_released |= res.drag_released();
                            });

                            // TODO: This is a hack to prevent the color from going out of range.
                            //       Update this when we have a proper way to constrain values.
                            value.r = value.r.clamp(0.0, 1.0);
                            value.g = value.g.clamp(0.0, 1.0);
                            value.b = value.b.clamp(0.0, 1.0);
                        } else {
                            ui.label("(unsupported type)");
                        }
                    });

                    if any_changed {
                        component
                            .set_field_by_name(field_name, field.to_owned())
                            .unwrap();
                    }
                }

                if any_clicked {
                    state
                        .begin_action(
                            Box::new(UpdateComponent::new(entity, component.type_id())),
                            commands,
                        )
                        .unwrap();
                }
                if any_released {
                    state.end_action::<UpdateComponent>(commands).unwrap();
                }
                if any_changed && !state.action_in_progress::<UpdateComponent>() {
                    state
                        .perform_action(UpdateComponent::new(entity, component.type_id()), commands)
                        .unwrap();
                }
            }
        }
    }
}
