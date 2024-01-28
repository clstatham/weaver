use weaver::{
    core::{renderer::compute::hdr_loader::HdrLoader, ui::builtin::FpsDisplay},
    prelude::*,
};

pub mod state;
pub mod ui;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let app = App::new(1600, 900)?;

    app.add_resource(state::EditorState::new(&app.world))?;
    app.add_resource(FpsDisplay::new())?;

    app.add_system_to_stage(Setup, SystemStage::Startup);

    app.add_system_to_stage(state::EditorActions, SystemStage::PreUpdate);

    app.add_system_to_stage(UpdateCamera, SystemStage::Update);
    app.add_system_to_stage(state::SelectedEntityDoodads, SystemStage::Update);

    app.add_system_to_stage(ui::FpsDisplayUi, SystemStage::PostUpdate);
    app.add_system_to_stage(ui::SceneTreeUi, SystemStage::PostUpdate);
    app.add_system_to_stage(ui::ScriptUpdateUi, SystemStage::PostUpdate);
    app.add_system_to_stage(state::EditorStateUi, SystemStage::PostUpdate);
    app.add_system_to_stage(state::PickEntity, SystemStage::PostUpdate);

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
        controller.update(&input, time.delta_seconds, &mut camera);
    }
}
