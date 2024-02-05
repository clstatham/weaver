use weaver::{
    core::{app::Window, renderer::compute::hdr_loader::HdrLoader},
    prelude::*,
};

pub mod state;
// pub mod ui;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let app = App::new(1600, 900)?;

    app.add_resource(state::EditorState::new(&app.world))?;

    app.add_system_to_stage(Setup, SystemStage::Startup);

    app.add_system_to_stage(EditorRender, SystemStage::Render);

    app.add_system_to_stage(UpdateCamera, SystemStage::PostRender);

    // app.add_script("assets/scripts/editor/main.loom");

    app.run()
}

pub struct Setup;

impl System for Setup {
    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        let skybox = {
            let world = world.read();
            let mut assets = world.write_resource::<AssetServer>().unwrap();
            let hdr_loader = world.read_resource::<HdrLoader>().unwrap();
            assets.load_skybox("sky_2k.hdr", &hdr_loader)
        };
        world.write().spawn(skybox).unwrap();

        let camera = Camera::default();
        let controller = FlyCameraController {
            speed: 10.0,
            sensitivity: 0.1,
            aspect: 1600.0 / 900.0,
            translation: Vec3::new(0.0, 0.0, 5.0),
            ..Default::default()
        };

        {
            let mut world = world.write();
            let e = world.spawn(camera).unwrap();
            world.add_component(e, controller).unwrap();
        }

        Ok(vec![])
    }

    fn reads(&self) -> Vec<TypeUid> {
        vec![]
    }

    fn writes(&self) -> Vec<TypeUid> {
        vec![]
    }
}

struct UpdateCamera;

impl System for UpdateCamera {
    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        let world = world.read();
        let input = world.read_resource::<Input>().unwrap();
        let time = world.read_resource::<Time>().unwrap();
        let query = world
            .query()
            .write::<Camera>()?
            .write::<FlyCameraController>()?
            .build();
        for results in query.iter() {
            let [ref mut camera, ref mut controller] = &mut results.into_vec()[..] else {
                unreachable!()
            };
            let camera = camera.get_mut::<Camera>().unwrap();
            let controller = controller.get_mut::<FlyCameraController>().unwrap();
            let aspect = controller.aspect;
            controller.update(&input, time.delta_seconds, aspect, camera);
        }
        Ok(vec![])
    }

    fn reads(&self) -> Vec<TypeUid> {
        vec![]
    }

    fn writes(&self) -> Vec<TypeUid> {
        vec![]
    }
}

struct EditorRender;

impl System for EditorRender {
    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        let world = world.read();
        let mut renderer = world.write_resource::<Renderer>().unwrap();
        let mut ui = world.write_resource::<EguiContext>().unwrap();
        let window = world.read_resource::<Window>().unwrap();
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
        Ok(vec![])
    }

    fn reads(&self) -> Vec<TypeUid> {
        vec![]
    }

    fn writes(&self) -> Vec<TypeUid> {
        vec![]
    }
}
