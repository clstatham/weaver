use weaver::{app::App, winit::WinitPlugin};

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut app = App::new()?;
    app.add_plugin(WinitPlugin)?;

    app.run()
}
