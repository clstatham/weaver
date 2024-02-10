use fabricate::{registry::StaticId, relationship::Relationship};
use state::EditorState;
use weaver::{
    core::{app::Window, renderer::compute::hdr_loader::HdrLoader},
    prelude::*,
};

pub mod state;
pub mod ui;

#[derive(Component, Clone, Copy)]
pub struct TransformParent;

impl Relationship for TransformParent {}

#[derive(Component, Clone, Copy)]
pub struct TransformChild;

impl Relationship for TransformChild {}

pub fn inherit_transform(parent: Entity, child: Entity) {
    parent.add_relative(TransformParent, child).unwrap();
    child.add_relative(TransformChild, parent).unwrap();
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let vsync = std::env::var("WEAVER_VSYNC") == Ok("1".to_string());
    let app = App::new("Weaver Editor", 1600, 900, vsync)?;

    app.add_resource(ui::Tabs::default())?;
    app.add_resource(EditorState::new())?;
    app.add_resource(ui::fps_counter::FpsDisplay::new())?;

    app.add_system_to_stage(Setup, SystemStage::Startup)?;

    app.add_system_to_stage(UpdateTransforms, SystemStage::PreUpdate)?;

    app.add_system_to_stage(PickEntity, SystemStage::Update)?;
    app.add_system_to_stage(UpdateCamera, SystemStage::Update)?;

    app.add_system_to_stage(ui::EditorStateUi, SystemStage::Ui)?;

    app.add_system_to_stage(DrawEditorDoodads, SystemStage::PreRender)?;

    app.add_system_to_stage(EditorRender, SystemStage::Render)?;

    app.add_script("assets/scripts/editor/main.loom");

    app.run()
}

pub struct Setup;

impl System for Setup {
    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        world.defer(|world, commands| {
            {
                let ctx = world.read_resource::<EguiContext>().unwrap();
                let renderer = world.read_resource::<Renderer>().unwrap();

                let viewport = renderer.main_viewport();
                let viewport = viewport.read();
                let view = viewport.color_view(renderer.resource_manager());

                let id = ctx.convert_texture(renderer.device(), &view);

                let mut state = world.write_resource::<EditorState>().unwrap();

                state.viewport_id = Some(id);
            }

            let skybox = {
                let mut assets = world.write_resource::<AssetServer>().unwrap();
                let hdr_loader = world.read_resource::<HdrLoader>().unwrap();
                assets.load_skybox("sky_2k.hdr", &hdr_loader)
            };
            commands.spawn((skybox,));

            let camera = Camera::perspective_lookat(
                Vec3::new(5.0, 5.0, 5.0),
                Vec3::ZERO,
                Vec3::Y,
                std::f32::consts::FRAC_PI_2,
                16.0 / 9.0,
                0.1,
                100.0,
            );
            let (_, rotation, translation) =
                camera.view_matrix.inverse().to_scale_rotation_translation();
            let controller = FlyCameraController {
                speed: 10.0,
                sensitivity: 0.1,
                translation,
                rotation,
                ..Default::default()
            };
            commands.spawn((camera, controller));

            let (mesh, material) = {
                let mut assets = world.write_resource::<AssetServer>().unwrap();
                let mesh = assets.load_mesh("meshes/monkey_2x.glb");
                let material = assets.load_material("materials/wood.glb");
                (mesh, material)
            };
            let transform = Transform::default();
            let e1 = commands.spawn((
                mesh.clone(),
                material.clone(),
                transform,
                GlobalTransform::default(),
            ));
            let e2 = commands.spawn((
                mesh.clone(),
                material.clone(),
                Transform::from_translation(Vec3::new(2.0, 0.0, 0.0)),
                GlobalTransform::default(),
            ));
            inherit_transform(e1, e2);

            Ok::<(), anyhow::Error>(())
        })??;

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
        world.defer(|world, _| {
            let input = world.read_resource::<Input>().unwrap();
            let time = world.read_resource::<Time>().unwrap();
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
                controller.update(&input, time.delta_seconds, aspect, camera);
            }

            Ok::<(), anyhow::Error>(())
        })??;
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
        world.defer(|world, _| {
            let mut renderer = world.write_resource::<Renderer>().unwrap();
            let mut ui = world.write_resource::<EguiContext>().unwrap();
            let window = world.read_resource::<Window>().unwrap();
            {
                let mut encoder = renderer.begin_render();
                renderer.prepare_components();
                renderer.prepare_passes();
                renderer.render_to_viewport(&mut encoder).unwrap();
                renderer.render_ui(&mut ui, &window, &mut encoder);
                renderer.end_render(encoder);
            }
        })?;
        Ok(vec![])
    }

    fn reads(&self) -> Vec<Entity> {
        vec![Window::static_type_id()]
    }

    fn writes(&self) -> Vec<Entity> {
        vec![Renderer::static_type_id(), EguiContext::static_type_id()]
    }
}

struct UpdateTransforms;

impl System for UpdateTransforms {
    fn reads(&self) -> Vec<Entity> {
        vec![Transform::static_type_id()]
    }

    fn writes(&self) -> Vec<Entity> {
        vec![GlobalTransform::static_type_id()]
    }

    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        world.defer(|world, _| {
            let query = world
                .query()
                .entity()
                .read::<Transform>()?
                .write::<GlobalTransform>()?
                .without_dynamic(Entity::new_wildcard::<TransformChild>())
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

                if let Some(children) =
                    world.get_relatives_id(entity, TransformParent::type_id().id())
                {
                    for child in children {
                        update_transforms_recurse(world, child, global);
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
        })??;
        Ok(vec![])
    }
}

fn update_transforms_recurse(world: &World, entity: Entity, parent_global: GlobalTransform) {
    let local = {
        let transform = world.get(entity, Transform::type_id());
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

        global.matrix = parent_global.matrix * local;
        *global
    };

    if let Some(children) = world.get_relatives_id(entity, TransformParent::type_id().id()) {
        for child in children {
            update_transforms_recurse(world, child, global);
        }
    }
}

pub struct DrawEditorDoodads;

impl System for DrawEditorDoodads {
    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        world.defer(|world, _| {
            let mut doodads = world.write_resource::<Doodads>().unwrap();

            let gray = Color::new(0.5, 0.5, 0.5, 1.0);

            let grid_size = 100;

            for i in -grid_size..=grid_size {
                doodads.push(Doodad::Line(Line::new(
                    Vec3::new(-grid_size as f32, 0.0, i as f32),
                    Vec3::new(grid_size as f32, 0.0, i as f32),
                    gray,
                )));
                doodads.push(Doodad::Line(Line::new(
                    Vec3::new(i as f32, 0.0, -grid_size as f32),
                    Vec3::new(i as f32, 0.0, grid_size as f32),
                    gray,
                )));
            }

            let state = world.read_resource::<EditorState>().unwrap();

            if let Some(ref selected) = state.selected_entity {
                let transform = selected.with_component_ref::<GlobalTransform, _>(|t| *t);
                let aabb = selected.with_component_ref::<Mesh, _>(|m| m.aabb());
                if let Some((transform, aabb)) = transform.zip(aabb) {
                    let aabb = aabb.transformed(transform);
                    let position = aabb.center();
                    let scale = aabb.size();
                    let color = Color::new(0.0, 1.0, 0.0, 1.0);
                    doodads.push(Doodad::WireCube(Cube::new(
                        position,
                        Quat::IDENTITY,
                        scale,
                        color,
                    )));
                }
            }

            Ok::<(), anyhow::Error>(())
        })??;

        Ok(vec![])
    }

    fn reads(&self) -> Vec<Entity> {
        vec![EditorState::static_type_id()]
    }

    fn writes(&self) -> Vec<Entity> {
        vec![Doodads::static_type_id()]
    }
}

pub struct PickEntity;

impl System for PickEntity {
    fn run(&self, world: LockedWorldHandle, _: &[Data]) -> anyhow::Result<Vec<Data>> {
        world.defer(|world, _| {
            let input = world.read_resource::<Input>().unwrap();
            let renderer = world.read_resource::<Renderer>().unwrap();

            let q = world
                .query()
                .read::<Camera>()?
                .read::<FlyCameraController>()?
                .build();
            let q = q.iter().next().unwrap();
            let camera = q.get::<Camera>().unwrap();

            if input.mouse_button_just_pressed(MouseButton::Left) {
                let (x, y) = input.mouse_position().unwrap().into();
                let viewport_rect = renderer.viewport_rect();
                let x = x - viewport_rect.x;
                let y = y - viewport_rect.y;
                let screen_position = Vec2::new(x, y);
                let ray = camera.screen_to_ray(
                    screen_position,
                    Vec2::new(viewport_rect.width, viewport_rect.height),
                );
                let query = world
                    .query()
                    .entity()
                    .read::<GlobalTransform>()?
                    .read::<Mesh>()?
                    .build();
                let mut closest = None;
                let mut closest_distance = f32::MAX;
                for result in query.iter() {
                    let entity = result.entity().unwrap();
                    let global = result.get::<GlobalTransform>().unwrap();
                    let mesh = result.get::<Mesh>().unwrap();
                    let bounding = mesh.bounding_sphere().transformed(*global);
                    if let Some(distance) = bounding.intersect_ray(ray) {
                        if distance < closest_distance {
                            closest = Some(entity);
                            closest_distance = distance;
                        }
                    }
                }
                if let Some(closest) = closest {
                    let mut state = world.write_resource::<EditorState>().unwrap();
                    if state.selected_entity != Some(closest.to_owned()) {
                        state.selected_entity = Some(closest);
                        state.selected_component = None;
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
        })??;

        Ok(vec![])
    }

    fn reads(&self) -> Vec<Entity> {
        vec![Input::static_type_id(), Window::static_type_id()]
    }

    fn writes(&self) -> Vec<Entity> {
        vec![EditorState::static_type_id()]
    }
}
