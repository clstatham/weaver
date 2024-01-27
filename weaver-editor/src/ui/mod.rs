use std::sync::Arc;

use egui::TextEdit;
use weaver::{
    core::{scripts::Scripts, ui::builtin::FpsDisplay},
    ecs::component::Data,
    ecs::registry::{DynamicId, Registry},
    prelude::*,
};

use crate::state::{EditorState, SelectComponent, SelectEntity, UpdateComponent};

pub mod syntax_highlighting;

#[system(FpsDisplayUi())]
pub fn fps_display_ui(ctx: Res<EguiContext>, fps_display: ResMut<FpsDisplay>) {
    ctx.draw_if_ready(|ctx| {
        fps_display.run_ui(ctx);
    });
}

pub fn traverse_tree(
    world: &World,
    commands: &mut Commands,
    entity: Entity,
    graph: &EntityGraph,
    state: &mut EditorState,
    ui: &mut egui::Ui,
) {
    let selection = if let Some(selected) = state.selected_entity() {
        entity == selected
    } else {
        false
    };
    if selection {
        ui.visuals_mut().override_text_color = Some(egui::Color32::LIGHT_BLUE);
    } else {
        ui.visuals_mut().override_text_color = None;
    }
    let entity_name = match state.entity_name(entity) {
        Some(name) => format!("{} ({})", name, entity.id()),
        None => {
            format!("({})", entity.id())
        }
    };
    let children = graph.get_children(entity);
    ui.vertical(|ui| {
        let collapsing = egui::CollapsingHeader::new(&entity_name).show(ui, |ui| {
            ui.visuals_mut().override_text_color = None;
            for component in world.components_iter(&entity) {
                if let Some(selected) = state.selected_component() {
                    if selected == component.type_id() && entity == state.selected_entity().unwrap()
                    {
                        ui.visuals_mut().override_text_color = Some(egui::Color32::LIGHT_BLUE);
                    }
                }
                let component_header =
                    egui::CollapsingHeader::new(component.name()).show(ui, |ui| {
                        if let Some(fields) = component.fields() {
                            for field in fields.iter() {
                                ui.label(field.name());
                            }
                        }
                    });
                if component_header.header_response.secondary_clicked() {
                    state
                        .perform_action(SelectComponent::new(entity, component.type_id()))
                        .unwrap();
                }

                ui.visuals_mut().override_text_color = None;
            }
            for child in children {
                traverse_tree(world, commands, child, graph, state, ui);
            }
        });
        if collapsing.header_response.secondary_clicked() {
            state.perform_action(SelectEntity::new(entity)).unwrap();
        }
        if collapsing
            .header_response
            .double_clicked_by(egui::PointerButton::Secondary)
        {
            state.begin_rename_entity(entity);
        }
        if collapsing.header_response.middle_clicked() && entity != Entity::PLACEHOLDER {
            commands.despawn_recursive(entity);
        }
    });
}

pub struct SceneTreeUi;

impl System for SceneTreeUi {
    fn run(&self, world: Arc<RwLock<World>>, _input: &[&Data]) -> anyhow::Result<()> {
        let mut commands = Commands::new(world.clone());
        let world_lock = world.read();
        {
            let ctx = world_lock.read_resource::<EguiContext>()?;
            let mut state = world_lock.write_resource::<EditorState>()?;
            let scene_tree = world_lock.read_resource::<EntityGraph>()?;
            scene_tree_ui(&world_lock, &ctx, &mut state, &scene_tree, &mut commands);
            component_inspector_ui(&world_lock, &ctx, &mut state);
        }

        drop(world_lock);
        commands.finalize(&mut world.write());
        Ok(())
    }

    fn resources_read(&self, registry: &Registry) -> Vec<DynamicId> {
        vec![
            registry.get_static::<EguiContext>(),
            registry.get_static::<EntityGraph>(),
        ]
    }

    fn resources_written(&self, registry: &Registry) -> Vec<DynamicId> {
        vec![registry.get_static::<EditorState>()]
    }

    fn components_read(&self, _registry: &Registry) -> Vec<DynamicId> {
        vec![]
    }

    fn components_written(&self, _registry: &Registry) -> Vec<DynamicId> {
        vec![]
    }

    fn is_exclusive(&self) -> bool {
        false
    }
}

pub fn scene_tree_ui(
    world: &World,
    ctx: &EguiContext,
    state: &mut EditorState,
    scene_tree: &EntityGraph,
    commands: &mut Commands,
) {
    ctx.draw_if_ready(|ctx| {
        egui::Window::new("Scene Tree").show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                egui::CollapsingHeader::new("Root").show(ui, |ui| {
                    for root in scene_tree.roots() {
                        traverse_tree(world, commands, root, scene_tree, state, ui);
                    }
                });
            });
        });
    });
}

pub fn component_inspector_ui(world: &World, ctx: &EguiContext, state: &mut EditorState) {
    ctx.draw_if_ready(|ctx| {
        egui::Window::new("Component Inspector").show(ctx, |ui| {
            if let Some(entity) = state.selected_entity() {
                if let Some(component) = state.selected_component() {
                    let component = world
                        .components
                        .entity_components_iter(entity.id())
                        .unwrap()
                        .find(|c| c.type_id() == component)
                        .unwrap();
                    ui.label(component.name());
                    if let Some(fields) = component.fields() {
                        let mut any_clicked = false;
                        let mut any_released = false;
                        let mut any_changed = false;
                        for field in fields.iter() {
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
                                .begin_action(Box::new(UpdateComponent::new(
                                    entity,
                                    component.type_id(),
                                )))
                                .unwrap();
                        }
                        if any_released {
                            state.end_action::<UpdateComponent>().unwrap();
                        }
                        if any_changed && !state.action_in_progress::<UpdateComponent>() {
                            state
                                .perform_action(UpdateComponent::new(entity, component.type_id()))
                                .unwrap();
                        }
                    }
                }
            }
        });
    });
}

#[system(ScriptUpdateUi())]
pub fn script_update_ui(ctx: Res<EguiContext>, scripts: Res<Scripts>) {
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
                        ui.add(
                            TextEdit::multiline(&mut script.content)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .layouter(&mut layouter),
                        );
                    });
                });
            });
        }
    });
}
