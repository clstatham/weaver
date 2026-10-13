Wishlist:
- More Entity utility (make Entities 90% of what you need) (partial: `Entities::reserve`/`is_alive` and deferred `Commands::spawn` returning an Entity, f468581/a57f5a1; no Entity-handle wrapper API)
- ~~BLOBs for component storage (instead of basically `Vec<Box<dyn Component>>`)~~ (done: f693ef9, type-erased `ComponentVec` in crates/weaver-ecs/src/component_vec.rs)
	- ~~Only do this if we need to for performance~~
- ~~Revisit change detection~~ (done: per-system last_run ticks, a57f5a1; removal detection via `RemovedComponents`; tick/Changed fixes in 5593f8b/f693ef9)
- Revisit reflection
	- serde?