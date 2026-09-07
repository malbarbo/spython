use crate::repl_reader::Theme;
use engine::Level;
use std::fs;
use std::path::PathBuf;

pub struct Config {
    pub level: Level,
    pub theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            level: Level::Functions,
            theme: Theme::Dark,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let mut path = dirs::config_dir()?;
    path.push("spython");
    path.push("config");
    Some(path)
}

pub fn load() -> Config {
    let mut config = Config::default();
    let Some(path) = config_path() else {
        return config;
    };
    let Ok(content) = fs::read_to_string(&path) else {
        return config;
    };
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "level" => {
                if let Some(level) = value.trim().parse::<u8>().ok().and_then(Level::from_u8) {
                    config.level = level;
                }
            }
            "theme" => {
                if let Some(theme) = Theme::parse(value.trim()) {
                    config.theme = theme;
                }
            }
            _ => {}
        }
    }
    config
}

pub fn save(level: Level, theme: Theme) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = format!("level={}\ntheme={}\n", level as u8, theme.name());
    let _ = fs::write(&path, content);
}
