use weaver::prelude::*;

use crate::{state::EditorState, TransformChild, TransformParent};

#[derive(Component, Clone)]
pub struct NameTag(pub String);

pub fn scene_tree_ui(world_handle: &LockedWorldHandle, state: &mut EditorState, ui: &mut egui::Ui) {
    world_handle
        .defer(|world, _| {
            egui::CollapsingHeader::new("World")
                .default_open(true)
                .show(ui, |ui| {
                    let q = world
                        .query()
                        .entity()
                        .without_dynamic(Entity::new_wildcard::<TransformChild>())
                        .unwrap()
                        .build();
                    for result in q.iter() {
                        let entity = result.entity().unwrap();
                        {
                            let name = entity
                                .with_component_ref::<NameTag, _>(world_handle, |tag| tag.0.clone())
                                .or_else(|| entity.type_name())
                                .unwrap_or_else(|| format!("{}", entity.id()));
                            scene_tree_ui_recurse(world_handle, world, state, ui, entity, &name);
                        }
                    }
                });
        })
        .unwrap();
}

fn scene_tree_ui_recurse(
    world_handle: &LockedWorldHandle,
    world: &World,
    state: &mut EditorState,
    ui: &mut egui::Ui,
    node: Entity,
    name: &str,
) {
    let text = if state.selected_entity == Some(node) {
        egui::RichText::new(name).strong().underline()
    } else {
        egui::RichText::new(name)
    };
    let id = ui.make_persistent_id(node);
    let header =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    header
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                let selected = state.selected_entity == Some(node);
                if ui
                    .add(egui::Label::new(text).sense(egui::Sense::click_and_drag()))
                    .clicked()
                    && !selected
                {
                    state.selected_entity = Some(node);
                    state.selected_component = None;
                }
                ui.with_layout(egui::Layout::right_to_left(Default::default()), |ui| {
                    if ui.button("Rename").clicked() {
                        state.entity_rename_buffer = name.to_string();
                        state.entity_being_renamed = Some(node);
                    }
                });
            });
        })
        .body(|ui| {
            let rels = world.get_relatives_id(node, TransformParent::type_id().id());
            if let Some(rels) = rels {
                for child in rels {
                    let name = child
                        .with_component_ref::<NameTag, _>(world_handle, |tag| tag.0.clone())
                        .or_else(|| child.type_name())
                        .unwrap_or_else(|| format!("{}", child.id()));
                    scene_tree_ui_recurse(world_handle, world, state, ui, child, &name);
                }
            }

            let arch = world.storage().entity_archetype(node).unwrap();
            let components = arch
                .row_type_filtered(node, |ty| {
                    !ty.is_relative() && ty.id() != NameTag::type_id().id()
                })
                .unwrap();
            for component in components {
                let ty = component.type_id();
                let name = ty
                    .type_name()
                    .unwrap_or_else(|| format!("[type {}]", ty.id()));

                let name = if state.selected_component == Some(component.entity()) {
                    egui::RichText::new(name).strong().underline()
                } else {
                    egui::RichText::new(name)
                };
                let selected = state.selected_component == Some(component.entity());
                let header = egui::SelectableLabel::new(selected, name);
                let response = ui.add(header);
                if response.clicked() {
                    state.selected_entity = Some(node);
                    state.selected_component = Some(component.entity());
                }
            }
        });
}
