pub mod app;
pub mod renderer;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    app::App::new((800, 600), "Weaver").run()
}
