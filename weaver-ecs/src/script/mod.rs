use std::path::{Path, PathBuf};

use self::parser::{LoomParser, Scope};

pub mod interp;
pub mod parser;

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
        let name = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .split('.')
            .next()
            .unwrap()
            .to_string();
        let content = std::fs::read_to_string(&path)?;
        Ok(Self::new(name, path, content))
    }

    pub fn is_parsed(&self) -> bool {
        !self.scopes.is_empty()
    }

    pub fn parse(&mut self) {
        if self.is_parsed() {
            return;
        }
        let mut parser = LoomParser::new();
        parser.parse_script(&self.content).unwrap();
        self.scopes = parser.finish();
    }
}
