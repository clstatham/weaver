use fabricate::world::BelongsToWorld;
use weaver::prelude::*;

use crate::{state::EditorState, InheritTransform};

pub fn scene_tree_ui(world: &World, state: &mut EditorState, ui: &mut egui::Ui) {
    egui::CollapsingHeader::new("World").show(ui, |ui| {
        let q = world
            .query()
            .entity()
            .with_dynamic(Entity::new_wildcard(BelongsToWorld::type_uid().id()))
            .unwrap()
            .build();
        for result in q.iter() {
            let entity = result.get_entity().unwrap();
            let name = entity
                .type_name()
                .unwrap_or_else(|| format!("{}", entity.id()));
            scene_tree_ui_recurse(world, state, ui, &entity, &name);
        }
    });
}

fn scene_tree_ui_recurse(
    world: &World,
    state: &mut EditorState,
    ui: &mut egui::Ui,
    node: &Entity,
    name: &str,
) {
    let text = if Some(*node) == state.selected_entity {
        egui::RichText::new(name).strong()
    } else {
        egui::RichText::new(name)
    };
    egui::CollapsingHeader::new(text)
        .id_source(node)
        .show(ui, |ui| {
            let rels = world.get_relatives(node, &InheritTransform::type_uid());
            if let Some(rels) = rels {
                for child in rels {
                    let name = child
                        .type_name()
                        .unwrap_or_else(|| format!("{}", child.id()));
                    scene_tree_ui_recurse(world, state, ui, &child, &name);
                }
            }

            let arch = world.storage().entity_archetype(node).unwrap();
            let components = arch
                .row_type_filtered(node, |ty| !ty.is_relative())
                .unwrap();
            for component in components {
                let ty = component.type_uid();
                let name = ty
                    .type_name()
                    .unwrap_or_else(|| format!("[type {}]", ty.id()));

                let name = if state.selected_component == Some(*component.value_uid()) {
                    egui::RichText::new(name).strong()
                } else {
                    egui::RichText::new(name)
                };
                let selected = state.selected_component == Some(*component.value_uid());
                let header = egui::SelectableLabel::new(selected, name);
                let response = ui.add(header);
                if response.clicked() {
                    state.selected_entity = Some(*node);
                    state.selected_component = Some(*component.value_uid());
                }
            }
        });
}
