use weaver::prelude::*;

use crate::state::{Despawn, EditorState, SelectComponent, SelectEntity, Spawn};

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
    let mut parents_to_selected = vec![];
    if let Some(selected) = state.selected_entity() {
        parents_to_selected.extend(graph.get_all_parents(selected));
        parents_to_selected.push(selected);
    }
    ui.vertical(|ui| {
        let id = ui.make_persistent_id(entity_name.clone());
        let collapsing = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            parents_to_selected.contains(&entity),
        )
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(entity_name);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let despawn_button = egui::Button::new(
                        egui::RichText::new("-").color(ui.visuals().extreme_bg_color),
                    )
                    .fill(ui.visuals().warn_fg_color);
                    if ui.add(despawn_button).clicked() {
                        state
                            .perform_action(Despawn::new(entity), commands)
                            .unwrap();
                    }

                    ui.add(egui::Separator::default().vertical());
                    if ui.button("+").clicked() {
                        state
                            .perform_action(Spawn::<()>::new(Some(entity), None), commands)
                            .unwrap();
                    }

                    ui.add(egui::Separator::default().vertical());

                    if ui.button("rn").clicked() {
                        state.begin_rename_entity(entity, commands);
                    }
                });
            });
        })
        .body(|ui| {
            ui.visuals_mut().override_text_color = None;
            for component in world.components_iter(&entity) {
                if component.type_id() == world.registry().get_static::<()>() {
                    continue;
                }
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
                        .perform_action(SelectComponent::new(entity, component.type_id()), commands)
                        .unwrap();
                }

                ui.visuals_mut().override_text_color = None;
            }
            for child in children {
                traverse_tree(world, commands, child, graph, state, ui);
            }
        });
        if collapsing.0.secondary_clicked() {
            state
                .perform_action(SelectEntity::new(entity), commands)
                .unwrap();
        }
        if collapsing
            .0
            .double_clicked_by(egui::PointerButton::Secondary)
        {
            state.begin_rename_entity(entity, commands);
        }
        if collapsing.0.middle_clicked() && entity != Entity::PLACEHOLDER {
            commands.despawn_recursive(entity);
        }
    });
}

pub fn scene_tree_ui(
    world: &World,
    ui: &mut egui::Ui,
    state: &mut EditorState,
    scene_tree: &EntityGraph,
    commands: &mut Commands,
) {
    egui::ScrollArea::both().show(ui, |ui| {
        egui::CollapsingHeader::new("Root")
            .default_open(true)
            .show(ui, |ui| {
                for root in scene_tree.roots() {
                    traverse_tree(world, commands, root, scene_tree, state, ui);
                }
            });
    });
}
