use std::path::{Path, PathBuf};

use self::parser::{LoomParser, Scope};

pub mod interp;
pub mod parser;
pub mod value;

#[derive(Clone)]
pub struct Script {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
    pub scopes: Vec<Scope>,
}

impl Script {
    pub fn new(name: String, path: PathBuf, content: String) -> Self {
        Self {
            name,
            path,
            content,
            scopes: Vec::new(),
        }
    }

    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let content = std::fs::read_to_string(&path)?;
        let mut parser = LoomParser::new();
        parser.parse_script(&content)?;
        let scopes = parser.finish();
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
