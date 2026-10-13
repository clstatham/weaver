Wishlist:
- 2d and 3d "frontends" with a unified "backend"
- ~~Batching as first-class feature~~ (done: a57f5a1, crates/weaver-renderer/src/phase.rs + mesh/instances.rs)
- Extendable and modular (like the rest of the engine)
- Integrates into ECS as much as possible (partial: render-world entities, `AsBindGroup`, `RenderAsset`, draw fns fetching params via `SystemState`, a57f5a1)
- Performance metrics (Resources?) (partial: `FrameTime`/`FrameTimeStats` resources + egui window in weaver-diagnostics, d911e44; no renderer-specific metrics)