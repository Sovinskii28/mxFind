use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub servers: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            servers: default_servers(),
        }
    }
}

pub fn default_servers() -> Vec<String> {
    vec![
        "matrix.org".to_string(),
        "envs.net".to_string(),
        "tchncs.de".to_string(),
        "kde.org".to_string(),
        "gnome.org".to_string(),
        "gitter.im".to_string(),
        "tchncs.de".to_string(),
        "feline.support".to_string(),
        "midov.pl".to_string(),
        "nitro.chat".to_string(),
        "continuwuity.org".to_string(),
    ]
}

pub fn load_config(path: Option<&Path>) -> anyhow::Result<Config> {
    if let Some(path) = path {
        return read_config(path);
    }

    let Some(path) = default_config_path() else {
        return Ok(Config::default());
    };

    if path.exists() {
        read_config(&path)
    } else {
        Ok(Config::default())
    }
}

fn read_config(path: &Path) -> anyhow::Result<Config> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;

    toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file {}", path.display()))
}

fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|path| path.join("mxfind").join("config.toml"))
}
