use fabricate::{registry::StaticId, relationship::Relationship};
use weaver::{
    core::{app::Window, renderer::compute::hdr_loader::HdrLoader},
    prelude::*,
};

pub mod state;
pub mod ui;

#[derive(Atom, Clone, Copy)]
pub struct TransformParent;

impl Relationship for TransformParent {}

#[derive(Atom, Clone, Copy)]
pub struct TransformChild;

impl Relationship for TransformChild {}

pub fn inherit_transform(parent: &Entity, child: &Entity) {
    parent.add_relative(TransformParent, child).unwrap();
    child.add_relative(TransformChild, parent).unwrap();
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let vsync = std::env::var("WEAVER_VSYNC") == Ok("1".to_string());
    let app = App::new("Weaver Editor", 1600, 900, vsync)?;

    app.add_resource(ui::Tabs::default())?;
    app.add_resource(state::EditorState::new())?;
    app.add_resource(ui::fps_counter::FpsDisplay::new())?;

    app.add_system_to_stage(Setup, SystemStage::Startup);

    app.add_system_to_stage(UpdateTransforms, SystemStage::PreUpdate);

    app.add_system_to_stage(UpdateCamera, SystemStage::Update);

    app.add_system_to_stage(ui::EditorStateUi, SystemStage::Ui);

    app.add_system_to_stage(EditorRender, SystemStage::Render);

    app.add_script("assets/scripts/editor/main.loom");

    app.run()
}

pub struct Setup;

impl System for Setup {
    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        let skybox = {
            let world = world.read();
            let mut assets = world.write_resource::<AssetServer>().unwrap();
            let assets = assets.as_mut::<AssetServer>().unwrap();
            let hdr_loader = world.read_resource::<HdrLoader>().unwrap();
            let hdr_loader = hdr_loader.as_ref::<HdrLoader>().unwrap();
            assets.load_skybox("sky_2k.hdr", hdr_loader)
        };
        world.write().spawn((skybox,)).unwrap();

        let camera = Camera::default();
        let controller = FlyCameraController {
            speed: 10.0,
            sensitivity: 0.1,
            translation: Vec3::new(0.0, 0.0, 5.0),
            ..Default::default()
        };
        world.write().spawn((camera, controller)).unwrap();

        let (mesh, material) = {
            let world = world.read();
            let mut assets = world.write_resource::<AssetServer>().unwrap();
            let assets = assets.as_mut::<AssetServer>().unwrap();
            let mesh = assets.load_mesh("meshes/monkey_2x.glb");
            let material = assets.load_material("materials/wood.glb");
            (mesh, material)
        };
        let transform = Transform::default();
        let e1 = world
            .write()
            .spawn((
                mesh.clone(),
                material.clone(),
                transform,
                GlobalTransform::default(),
            ))
            .unwrap();
        let e2 = world
            .write()
            .spawn((
                mesh.clone(),
                material.clone(),
                Transform::from_translation(Vec3::new(2.0, 0.0, 0.0)),
                GlobalTransform::default(),
            ))
            .unwrap();
        inherit_transform(&e1, &e2);

        Ok(vec![])
    }

    fn reads(&self) -> Vec<Entity> {
        vec![]
    }

    fn writes(&self) -> Vec<Entity> {
        vec![]
    }
}

struct UpdateCamera;

impl System for UpdateCamera {
    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        let world = world.read();
        let input = world.read_resource::<Input>().unwrap();
        let input = input.as_ref::<Input>().unwrap();
        let time = world.read_resource::<Time>().unwrap();
        let time = time.as_ref::<Time>().unwrap();
        let query = world
            .query()
            .write::<Camera>()?
            .write::<FlyCameraController>()?
            .build();
        for results in query.iter() {
            let [ref mut camera, ref mut controller] = &mut results.into_inner()[..] else {
                unreachable!()
            };
            let camera = camera.get_mut::<Camera>().unwrap();
            let controller = controller.get_mut::<FlyCameraController>().unwrap();
            let aspect = controller.aspect;
            controller.update(input, time.delta_seconds, aspect, camera);
        }
        Ok(vec![])
    }

    fn reads(&self) -> Vec<Entity> {
        vec![]
    }

    fn writes(&self) -> Vec<Entity> {
        vec![]
    }
}

struct EditorRender;

impl System for EditorRender {
    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        let world = world.read();
        let mut renderer = world.write_resource::<Renderer>().unwrap();
        let renderer = renderer.as_mut::<Renderer>().unwrap();
        let mut ui = world.write_resource::<EguiContext>().unwrap();
        let ui = ui.as_mut::<EguiContext>().unwrap();
        let window = world.read_resource::<Window>().unwrap();
        let window = window.as_ref::<Window>().unwrap();
        {
            let mut encoder = renderer.begin_render();
            renderer.render_ui(ui, window, &mut encoder);
            renderer.prepare_components();
            renderer.prepare_passes();
            renderer.render_to_viewport(&mut encoder).unwrap();
            renderer.render_viewport_to_screen(&mut encoder).unwrap();
            renderer.end_render(encoder);
        }
        Ok(vec![])
    }

    fn reads(&self) -> Vec<Entity> {
        vec![Window::static_type_uid()]
    }

    fn writes(&self) -> Vec<Entity> {
        vec![Renderer::static_type_uid(), EguiContext::static_type_uid()]
    }
}

struct UpdateTransforms;

impl System for UpdateTransforms {
    fn reads(&self) -> Vec<Entity> {
        vec![Transform::static_type_uid()]
    }

    fn writes(&self) -> Vec<Entity> {
        vec![GlobalTransform::static_type_uid()]
    }

    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        let world = world.read();
        let query = world
            .query()
            .entity()
            .read::<Transform>()?
            .write::<GlobalTransform>()?
            .without_dynamic(&Entity::new_wildcard::<TransformChild>())
            .unwrap()
            .build();

        for result in query.iter() {
            let [entity, ref transform, ref mut global] = &mut result.into_inner()[..] else {
                unreachable!()
            };
            let entity = entity.entity();
            let transform = transform.get::<Transform>().unwrap();
            let global = global.get_mut::<GlobalTransform>().unwrap();

            let local = {
                Mat4::from_scale_rotation_translation(
                    transform.scale,
                    transform.rotation,
                    transform.translation,
                )
            };

            let global = {
                global.matrix = local;
                *global
            };

            if let Some(children) = world.get_relatives_id(entity, TransformParent::type_uid().id())
            {
                for child in children {
                    update_transforms_recurse(&world, &child, global);
                }
            }
        }
        Ok(vec![])
    }
}

fn update_transforms_recurse(world: &World, entity: &Entity, parent_global: GlobalTransform) {
    let local = {
        let transform = world.get(entity, &Transform::type_uid());
        if transform.is_none() {
            return;
        }
        let transform = transform.unwrap();
        let transform = transform.as_ref::<Transform>().unwrap();

        Mat4::from_scale_rotation_translation(
            transform.scale,
            transform.rotation,
            transform.translation,
        )
    };

    let global = {
        let mut global = world.get_component_mut::<GlobalTransform>(entity).unwrap();
        let global = global.as_mut::<GlobalTransform>().unwrap();

        global.matrix = parent_global.matrix * local;
        *global
    };

    if let Some(children) = world.get_relatives_id(entity, TransformParent::type_uid().id()) {
        for child in children {
            update_transforms_recurse(world, &child, global);
        }
    }
}
