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

/// Save the level, leaving the rest of the file as it is.
pub fn save_level(level: Level) {
    save("level", &(level as u8).to_string());
}

/// Save the theme, leaving the rest of the file as it is. The level of the
/// session stays out, since a `--level` on the command line holds only for
/// that session.
pub fn save_theme(theme: Theme) {
    save("theme", theme.name());
}

fn save(key: &str, value: &str) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = fs::read_to_string(&path).unwrap_or_default();
    let _ = fs::write(&path, with_key(&content, key, value));
}

/// `content` with `key` set to `value`, on the line of its first occurrence
/// or on a new last line. A later line with the same key would win over it
/// in [`load`], so it goes.
fn with_key(content: &str, key: &str, value: &str) -> String {
    let mut out = String::new();
    let mut written = false;
    for line in content.lines() {
        let same_key = line.split_once('=').is_some_and(|(k, _)| k.trim() == key);
        if !same_key {
            out.push_str(line);
            out.push('\n');
        } else if !written {
            out.push_str(&format!("{key}={value}\n"));
            written = true;
        }
    }
    if !written {
        out.push_str(&format!("{key}={value}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_key_changes_only_its_own_line() {
        assert_eq!(
            with_key("level=2\ntheme=dark\n", "theme", "light"),
            "level=2\ntheme=light\n"
        );
    }

    #[test]
    fn with_key_adds_a_key_the_file_lacks() {
        assert_eq!(with_key("", "theme", "light"), "theme=light\n");
        assert_eq!(
            with_key("level=2", "theme", "light"),
            "level=2\ntheme=light\n"
        );
    }

    #[test]
    fn with_key_drops_a_later_line_of_the_same_key() {
        assert_eq!(
            with_key("theme=dark\nlevel=1\n theme = light\n", "theme", "dark"),
            "theme=dark\nlevel=1\n"
        );
    }
}
