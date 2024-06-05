use weaver::{core::renderer::compute::hdr_loader::HdrLoader, prelude::*};

#[derive(Default)]
struct State {
    viewport_id: Option<egui::TextureId>,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let vsync = std::env::var("WEAVER_VSYNC") == Ok("1".to_string());
    let app = App::new("Weaver Editor", 1600, 900, vsync)?;

    app.add_resource(State::default());

    app.add_system(setup, SystemStage::Init)?;

    app.add_system(update_camera, SystemStage::Update)?;

    app.add_system(ui, SystemStage::Ui)?;

    app.run()
}

fn setup(scene: &Scene) -> anyhow::Result<()> {
    let ctx = scene.world().get_resource::<EguiContext>().unwrap();
    let renderer = scene.world().get_resource::<Renderer>().unwrap();
    let viewport = renderer.main_viewport().read();
    let view = viewport.color_view(renderer.resource_manager());
    let id = ctx.convert_texture(renderer.device(), &view);
    let mut state = scene.world().get_resource_mut::<State>().unwrap();
    state.viewport_id = Some(id);

    let camera = Camera::perspective_lookat(
        Vec3::new(5.0, 5.0, 5.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        45.0,
        16.0 / 9.0,
        0.1,
        100.0,
    );
    let (_, rotation, translation) = camera.view_matrix.inverse().to_scale_rotation_translation();
    let camera = scene.create_node_with(camera);
    scene.world().insert_component(
        camera.entity(),
        FlyCameraController {
            speed: 10.0,
            sensitivity: 0.1,
            translation,
            rotation,
            ..Default::default()
        },
    );

    let mut asset_server = scene.world().get_resource_mut::<AssetServer>().unwrap();
    let hdr_loader = scene.world().get_resource::<HdrLoader>().unwrap();

    let skybox = asset_server.load_skybox("sky_2k.hdr", &hdr_loader);
    scene.create_node_with(skybox);

    let mesh = asset_server.load_mesh("meshes/cube.obj");
    let material = asset_server.load_material("materials/wood.glb");

    let cube = scene.create_node_with(GlobalTransform::default());
    scene
        .world()
        .insert_component(cube.entity(), Transform::default());
    scene.world().insert_component(cube.entity(), mesh);
    scene.world().insert_component(cube.entity(), material);

    Ok(())
}

fn update_camera(scene: &Scene) -> anyhow::Result<()> {
    let input = scene.world().get_resource::<Input>().unwrap();
    let time = scene.world().get_resource::<Time>().unwrap();

    let camera_query = scene.world().query(
        &Query::new()
            .write::<Camera>()
            .write::<FlyCameraController>(),
    );

    for entity in camera_query.iter() {
        let mut camera = camera_query.get_mut::<Camera>(entity).unwrap();
        let mut controller = camera_query.get_mut::<FlyCameraController>(entity).unwrap();
        let aspect = controller.aspect;
        controller.update(&input, time.delta_seconds, aspect, &mut camera);
    }

    Ok(())
}

fn ui(scene: &Scene) -> anyhow::Result<()> {
    let ctx_res = scene.world().get_resource::<EguiContext>().unwrap();

    ctx_res.draw_if_ready(|ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let renderer = scene.world().get_resource::<Renderer>().unwrap();

            let camera_query = scene
                .world()
                .query(&Query::new().read::<Camera>().read::<FlyCameraController>());
            let camera_entity = camera_query.iter().next().unwrap();

            let rect = ui.min_rect();

            renderer.set_viewport_rect(rect.into());
            camera_query
                .get_mut::<FlyCameraController>(camera_entity)
                .unwrap()
                .aspect = rect.aspect_ratio();

            let view = renderer
                .main_viewport()
                .read()
                .color_view(renderer.resource_manager());

            let state = scene.world().get_resource::<State>().unwrap();

            if let Some(id) = state.viewport_id {
                ctx_res.update_texture(renderer.device(), &view, id);

                ui.image((id, rect.size()));
            }
        });
    });

    Ok(())
}
