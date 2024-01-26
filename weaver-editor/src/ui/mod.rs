use std::sync::Arc;

use egui::TextEdit;
use weaver::{
    core::{scripts::Scripts, ui::builtin::FpsDisplay},
    ecs::component::Data,
    ecs::registry::{DynamicId, Registry},
    prelude::{parking_lot::RwLock, *},
};

use crate::state::EditorState;

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
    let selection = if let Some(selected) = state.selected_entity {
        entity == selected
    } else {
        false
    };
    if selection {
        ui.visuals_mut().override_text_color = Some(egui::Color32::LIGHT_BLUE);
    } else {
        ui.visuals_mut().override_text_color = None;
    }
    let entity_name = format!("Entity {}", entity.id());
    let children = graph.get_children(entity);
    ui.vertical(|ui| {
        let collapsing = egui::CollapsingHeader::new(&entity_name).show(ui, |ui| {
            for child in children {
                traverse_tree(world, commands, child, graph, state, ui);
            }
            ui.visuals_mut().override_text_color = None;
            for component in world.components_iter(&entity) {
                ui.label(component.name());
            }
        });
        if collapsing.header_response.secondary_clicked() {
            state.selected_entity = Some(entity);
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

#[system(ScriptUpdateUi())]
pub fn script_update_ui(ctx: Res<EguiContext>, scripts: Res<Scripts>, input: Res<Input>) {
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
                        let mut editor = ui.add(
                            TextEdit::multiline(&mut script.content)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .layouter(&mut layouter),
                        );
                        if editor.lost_focus() {
                            script.save().unwrap();
                            input.enable_input();
                        }
                        if editor.has_focus() {
                            input.disable_input();
                        }
                    });
                });
            });
        }
    });
}
