use std::path::{Path, PathBuf};

use anyhow::Result;

use self::parser::{LoomParser, Scope};

pub mod interp;
pub mod parser;
pub mod value;

#[derive(Clone)]
pub struct Script {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
    scopes: Vec<Scope>,
}

impl Script {
    pub fn new(name: String, path: PathBuf, content: String) -> Result<Self> {
        let mut this = Self {
            name,
            path,
            content,
            scopes: Vec::new(),
        };
        let mut parser = LoomParser::new();
        parser.parse_script(&this.content).unwrap();
        this.scopes = parser.finish()?;
        Ok(this)
    }

    #[allow(clippy::unwrap_used)]
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let content = std::fs::read_to_string(&path)?;
        let mut parser = LoomParser::new();
        parser.parse_script(&content)?;
        let scopes = parser.finish()?;
        Ok(Self {
            name,
            path,
            content,
            scopes,
        })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::write(&self.path, &self.content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::World, system::SystemStage, world::get_world};

    use super::*;
    #[test]
    fn test_script() {
        let world = get_world();
        let script = Script::load("src/script/test-scripts/test1.loom").unwrap();
        world.add_script(script);

        world.run_systems(SystemStage::Startup);
    }
}
