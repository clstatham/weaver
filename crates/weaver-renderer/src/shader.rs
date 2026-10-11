//! WGSL shaders, composed with naga_oil.
//!
//! [`ShaderLibrary`] reads every `.wgsl` file under the shader directory once, indexed by its
//! `#define_import_path`. Loading a shader registers just the modules it (transitively) imports,
//! then composes it.

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor};
use weaver_ecs::world::{ConstructFromWorld, World};
use weaver_util::prelude::*;

/// Where engine assets live: `$WEAVER_ASSET_DIR` if set, otherwise `./assets` if it exists,
/// otherwise the workspace's `assets` directory (as seen when this crate was built).
pub fn asset_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("WEAVER_ASSET_DIR") {
        return PathBuf::from(dir);
    }
    let local = PathBuf::from("assets");
    if local.is_dir() {
        return local;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

struct ShaderSource {
    path: PathBuf,
    source: String,
    import_path: Option<String>,
    imports: Vec<String>,
}

/// All WGSL sources under a directory, composable on demand.
pub struct ShaderLibrary {
    root: PathBuf,
    sources: Vec<ShaderSource>,
    composer: Lock<Composer>,
    /// Import paths already registered with the composer.
    registered: Lock<FxHashSet<String>>,
}

impl ShaderLibrary {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let mut sources = Vec::new();
        collect_sources(&root, &mut sources)
            .map_err(|e| anyhow!("failed to read shaders from {}: {e}", root.display()))?;
        Ok(Self {
            root,
            sources,
            composer: Lock::new(Composer::non_validating()),
            registered: Lock::new(FxHashSet::default()),
        })
    }

    /// Composes the shader at `path` (relative to the shader directory) into a module.
    ///
    /// Panics with the composer's error if the shader doesn't compile: shaders ship with the
    /// engine, so a broken one is a bug to fix rather than a condition to handle.
    pub fn load(&self, device: &wgpu::Device, path: &str) -> wgpu::ShaderModule {
        let module = self
            .compose(path)
            .unwrap_or_else(|e| panic!("failed to compile shader {path}: {e}"));
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(path),
            source: wgpu::ShaderSource::Naga(Cow::Owned(module)),
        })
    }

    fn compose(&self, path: &str) -> Result<wgpu::naga::Module> {
        let full_path = self.root.join(path);
        let shader = self
            .sources
            .iter()
            .find(|source| source.path == full_path)
            .ok_or_else(|| anyhow!("no shader at {}", full_path.display()))?;

        for import in &shader.imports {
            self.register(import, &mut Vec::new())?;
        }

        let mut composer = self.composer.write();
        let result = composer.make_naga_module(NagaModuleDescriptor {
            file_path: &shader.path.to_string_lossy(),
            source: &shader.source,
            ..Default::default()
        });
        result.map_err(|e| anyhow!("{}", e.emit_to_string(&composer)))
    }

    /// Registers the module with import path `import` (and, first, everything it imports).
    fn register(&self, import: &str, stack: &mut Vec<String>) -> Result<()> {
        if self.registered.read().contains(import) {
            return Ok(());
        }
        ensure!(
            !stack.iter().any(|entry| entry == import),
            "import cycle: {} -> {import}",
            stack.join(" -> ")
        );
        let module = self
            .sources
            .iter()
            .find(|source| source.import_path.as_deref() == Some(import))
            .ok_or_else(|| anyhow!("no shader defines import path `{import}`"))?;

        stack.push(import.to_string());
        for dependency in &module.imports {
            self.register(dependency, stack)?;
        }
        stack.pop();

        let mut composer = self.composer.write();
        let result = composer
            .add_composable_module(ComposableModuleDescriptor {
                file_path: &module.path.to_string_lossy(),
                source: &module.source,
                ..Default::default()
            })
            // the Ok value borrows the composer
            .map(|_| ());
        result.map_err(|e| anyhow!("{}", e.emit_to_string(&composer)))?;
        self.registered.write().insert(import.to_string());
        Ok(())
    }

    /// Resolves an `#import` target to the import path of a known module: `a::b::{X, Y}`,
    /// `a::b::X` and `a::b as c` all import `a::b`.
    fn resolve_import(&self, target: &str) -> Option<String> {
        let target = target.split(['{', ' ']).next()?.trim_end_matches("::");
        let mut candidate = target;
        loop {
            if self
                .sources
                .iter()
                .any(|source| source.import_path.as_deref() == Some(candidate))
            {
                return Some(candidate.to_string());
            }
            candidate = &candidate[..candidate.rfind("::")?];
        }
    }
}

impl ConstructFromWorld for ShaderLibrary {
    fn from_world(_world: &World) -> Self {
        let root = asset_root().join("shaders");
        let mut library = ShaderLibrary::new(&root).unwrap_or_else(|e| panic!("{e}"));
        library.resolve_all_imports();
        library
    }
}

impl ShaderLibrary {
    /// Turns each source's raw `#import` targets into import paths.
    fn resolve_all_imports(&mut self) {
        let resolved = self
            .sources
            .iter()
            .map(|source| {
                source
                    .imports
                    .iter()
                    .filter_map(|target| {
                        let resolved = self.resolve_import(target);
                        if resolved.is_none() {
                            log::warn!(
                                "{}: can't resolve `#import {target}`",
                                source.path.display()
                            );
                        }
                        resolved
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for (source, imports) in self.sources.iter_mut().zip(resolved) {
            source.imports = imports;
        }
    }
}

fn collect_sources(dir: &Path, sources: &mut Vec<ShaderSource>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_sources(&path, sources)?;
        } else if path.extension().is_some_and(|ext| ext == "wgsl") {
            let source = std::fs::read_to_string(&path)?;
            let mut import_path = None;
            let mut imports = Vec::new();
            for line in source.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("#define_import_path") {
                    import_path = Some(rest.trim().to_string());
                } else if let Some(rest) = line.strip_prefix("#import") {
                    imports.push(rest.trim().trim_end_matches(';').to_string());
                }
            }
            sources.push(ShaderSource {
                path,
                source,
                import_path,
                imports,
            });
        }
    }
    Ok(())
}
