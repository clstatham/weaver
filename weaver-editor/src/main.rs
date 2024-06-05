use weaver::prelude::*;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let vsync = std::env::var("WEAVER_VSYNC") == Ok("1".to_string());
    let app = App::new("Weaver Editor", 1600, 900, vsync)?;

    app.run()
}
