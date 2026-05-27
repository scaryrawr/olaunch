use std::path::PathBuf;

use crate::error::{OlaunchError, Result};

#[derive(Clone, Debug)]
pub struct Paths {
    home: PathBuf,
}

impl Paths {
    pub fn detect() -> Result<Self> {
        let home = directories::BaseDirs::new()
            .map(|dirs| dirs.home_dir().to_path_buf())
            .ok_or_else(|| OlaunchError::Message("could not determine home directory".into()))?;
        Ok(Self { home })
    }

    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    pub fn home(&self) -> &PathBuf {
        &self.home
    }

    pub fn codex_config(&self) -> PathBuf {
        self.home.join(".codex").join("config.toml")
    }

    pub fn codex_app_model_catalog(&self) -> PathBuf {
        self.home
            .join(".codex")
            .join("olaunch-codex-app-models.json")
    }

    pub fn hermes_config(&self) -> PathBuf {
        self.home.join(".hermes").join("config.yaml")
    }

    pub fn opencode_local_binary(&self) -> PathBuf {
        let name = if cfg!(windows) {
            "opencode.exe"
        } else {
            "opencode"
        };
        self.home.join(".opencode").join("bin").join(name)
    }

    pub fn opencode_model_state(&self) -> PathBuf {
        self.home
            .join(".local")
            .join("state")
            .join("opencode")
            .join("model.json")
    }

    pub fn claude_local_binary(&self) -> PathBuf {
        let name = if cfg!(windows) {
            "claude.exe"
        } else {
            "claude"
        };
        self.home.join(".claude").join("local").join(name)
    }
}
