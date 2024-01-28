use std::sync::Arc;

use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use weaver::{core::scripts::Scripts, prelude::*};

use crate::state::{EditorState, RenameEntity};

use self::fps_counter::FpsDisplay;

pub mod component_inspector;
pub mod fps_counter;
pub mod scene_tree;
pub mod script;
pub mod syntax_highlighting;

pub type Tab = String;

#[derive(Component)]
pub struct Tabs {
    pub(crate) tree: DockState<Tab>,
}

impl Default for Tabs {
    fn default() -> Self {
        let mut tree = DockState::new(vec![]);
        let [left, right] = tree.main_surface_mut().split_left(
            NodeIndex::root(),
            0.3,
            vec!["Scene Tree".to_string()],
        );

        tree.main_surface_mut()
            .split_below(right, 0.5, vec!["Assets".to_string()]);

        let [top, _bottom] =
            tree.main_surface_mut()
                .split_above(left, 0.3, vec!["Component Inspector".to_string()]);

        tree.main_surface_mut()
            .split_above(right, 0.3, vec!["Console".to_string()]);

        tree.main_surface_mut()
            .split_above(top, 1.0, vec!["Viewport".to_string()]);

        Self { tree }
    }
}

pub struct EditorTabViewer<'a> {
    world: Arc<RwLock<World>>,
    commands: &'a mut Commands,
    state: &'a mut EditorState,
}

impl<'a> TabViewer for EditorTabViewer<'a> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.as_str().into()
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        if tab.as_str() == "Viewport" {
            let world = self.world.read();
            let renderer = world.read_resource::<Renderer>().unwrap();
            renderer.set_viewport_enabled(false);
        }
        true
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        let world_lock = self.world.read();
        {
            let renderer = world_lock.read_resource::<Renderer>().unwrap();
            let scene_tree = world_lock.read_resource::<EntityGraph>().unwrap();
            match tab.as_str() {
                "Viewport" => {
                    // render to the viewport

                    ui.label("Viewport");

                    let camera = world_lock.query::<&mut FlyCameraController>();
                    let mut camera = camera.iter().next().unwrap();
                    let rect = ui.min_rect().into();
                    renderer.set_viewport_rect(rect);
                    camera.aspect = rect.width / rect.height;
                }
                "Scene Tree" => {
                    scene_tree::scene_tree_ui(
                        &world_lock,
                        ui,
                        self.state,
                        &scene_tree,
                        self.commands,
                    );
                }
                "Component Inspector" => {
                    component_inspector::component_inspector_ui(
                        &world_lock,
                        ui,
                        self.state,
                        self.commands,
                    );
                }
                "Console" => {
                    let mut scripts = world_lock.write_resource::<Scripts>().unwrap();
                    script::script_console_ui(ui, &mut scripts);
                }
                // "Script" => {
                //     let mut scripts = world_lock.write_resource::<Scripts>().unwrap();
                //     script::script_edit_ui(ui, &mut scripts);
                // }
                tab => {
                    ui.label(tab);
                }
            }
        }
    }
}

#[system(EditorStateUi())]
pub fn editor_state_ui(
    state: ResMut<EditorState>,
    ctx: Res<EguiContext>,
    tree: ResMut<Tabs>,
    mut commands: Commands,
    fps: ResMut<FpsDisplay>,
) {
    let world = state.world.clone();

    ctx.draw_if_ready(|ctx| {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            fps.run_ui(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            DockArea::new(&mut tree.tree)
                .style(Style::from_egui(ctx.style().as_ref()))
                .show_inside(
                    ui,
                    &mut EditorTabViewer {
                        world,
                        commands: &mut commands,
                        state: &mut state,
                    },
                );
        });

        if state.show_rename_entity {
            egui::Window::new("Rename Entity").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    let response = ui.text_edit_singleline(&mut state.entity_rename_buffer);
                    if response.lost_focus() {
                        state.show_rename_entity = false;
                        state.end_action::<RenameEntity>(&mut commands).unwrap();
                    }
                });
            });
        }
    })
}
