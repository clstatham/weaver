//! WESL shaders, compiled with the `wesl` crate.
//!
//! The shader directory is a WESL package: `import package::utils::Foo;` resolves to
//! `utils.wesl` in the shader directory. Loading a shader compiles it and everything it
//! (transitively) imports into a single WGSL module.

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use weaver_ecs::world::{ConstructFromWorld, World};
use weaver_util::prelude::*;
use wesl::{CompileOptions, Compiler, resolver::FileResolver, syntax::ModulePath};

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

/// The WESL package rooted at a directory, compiled on demand.
pub struct ShaderLibrary {
    compiler: Compiler<FileResolver>,
}

impl ShaderLibrary {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            compiler: Compiler::new_with_resolver(
                CompileOptions::default(),
                FileResolver::new(root),
            ),
        }
    }

    /// Compiles the shader at `path` (relative to the shader directory) into a module.
    ///
    /// Panics with the compiler's error if the shader doesn't compile: shaders ship with the
    /// engine, so a broken one is a bug to fix rather than a condition to handle.
    pub fn load(&self, device: &wgpu::Device, path: &str) -> wgpu::ShaderModule {
        let module = self
            .compile(path)
            .unwrap_or_else(|e| panic!("failed to compile shader {path}: {e}"));
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(path),
            source: wgpu::ShaderSource::Wgsl(Cow::Owned(module)),
        })
    }

    fn compile(&self, path: &str) -> Result<String> {
        let module = path
            .strip_suffix(".wesl")
            .unwrap_or(path)
            .replace('/', "::");
        let module: ModulePath = format!("package::{module}")
            .parse()
            .map_err(|e| anyhow!("invalid shader path {path}: {e}"))?;
        let result = self
            .compiler
            .compile_module(&module)
            .map_err(|e| anyhow!("{e}"))?;
        Ok(result.syntax.to_string())
    }
}

impl ConstructFromWorld for ShaderLibrary {
    fn from_world(_world: &World) -> Self {
        let root = asset_root().join("shaders");
        ShaderLibrary::new(&root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every entry-point shader compiles to WGSL that naga accepts.
    #[test]
    fn all_shaders_compile() {
        let root = asset_root().join("shaders");
        let library = ShaderLibrary::new(&root);
        for entry in std::fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if !name.ends_with(".wesl") || name == "utils.wesl" || name == "package.wesl" {
                continue;
            }
            let wgsl = library
                .compile(&name)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let module = wgpu::naga::front::wgsl::parse_str(&wgsl)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&wgsl)));
            assert!(!module.entry_points.is_empty(), "{name}: no entry points");
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        }
    }
}
