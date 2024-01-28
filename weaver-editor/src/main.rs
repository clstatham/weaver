use ui::{fps_counter::FpsDisplay, EditorStateUi, Tabs};
use weaver::{
    core::{app::Window, renderer::compute::hdr_loader::HdrLoader},
    prelude::*,
};

pub mod state;
pub mod ui;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let app = App::new(1600, 900)?;

    app.add_resource(FpsDisplay::new())?;
    app.add_resource(state::EditorState::new(&app.world))?;
    app.add_resource(Tabs::default())?;

    app.add_system_to_stage(Setup, SystemStage::Startup);

    app.add_system_to_stage(state::EditorActions, SystemStage::PreUpdate);

    app.add_system_to_stage(state::SelectedEntityDoodads, SystemStage::Update);

    app.add_system_to_stage(state::PickEntity, SystemStage::PostUpdate);

    app.add_system_to_stage(EditorRenderUi, SystemStage::Render);
    app.add_system_to_stage(EditorStateUi, SystemStage::PostRender);

    app.add_system_to_stage(UpdateCamera, SystemStage::PostRender);

    app.add_script("assets/scripts/editor/main.loom");

    app.run()
}

#[system(Setup())]
fn setup(commands: Commands, assets: ResMut<AssetServer>, hdr_loader: Res<HdrLoader>) {
    commands.spawn(assets.load_skybox("sky_2k.hdr", &hdr_loader));

    let camera = Camera::default();
    let controller = FlyCameraController {
        speed: 10.0,
        sensitivity: 0.1,
        aspect: 1600.0 / 900.0,
        translation: Vec3::new(0.0, 0.0, 5.0),
        ..Default::default()
    };

    commands.spawn((camera, controller));

    // commands.spawn(PointLight::new(
    //     Vec3::new(10.0, 10.0, 10.0),
    //     Color::WHITE,
    //     100.0,
    //     100.0,
    // ));
}

#[system(UpdateCamera())]
fn update_camera(
    input: Res<Input>,
    time: Res<Time>,
    mut query: Query<(&mut Camera, &mut FlyCameraController)>,
) {
    for (mut camera, mut controller) in query.iter() {
        let aspect = controller.aspect;
        controller.update(&input, time.delta_seconds, aspect, &mut camera);
    }
}

#[system(EditorRenderUi())]
fn editor_render_ui(renderer: ResMut<Renderer>, ui: ResMut<EguiContext>, window: Res<Window>) {
    if let Some(mut encoder) = renderer.begin_render() {
        renderer.render_ui(&mut ui, &window, &mut encoder);
        if renderer.viewport_enabled() {
            renderer.prepare_components();
            renderer.prepare_passes();
            renderer.render_to_viewport(&mut encoder).unwrap();
            renderer.render_viewport_to_screen(&mut encoder).unwrap();
        }
        renderer.end_render(encoder);
    }
}
