use crate::app::App;

pub fn editor_main() -> anyhow::Result<()> {
    let mut app = App::new((800, 600), "Weaver");
    app.renderer.camera.position = glam::Vec3::new(1.0, 1.0, 1.0);

    app.world = serde_yaml::from_str(std::fs::read_to_string("assets/editor.yaml")?.as_str())?;

    app.run()?;

    Ok(())
}
