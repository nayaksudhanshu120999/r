use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const DEFAULT_CONFIG_TOML: &str = r#"[bangs]
yt = "https://www.youtube.com/results?search_query={q}"
g = "https://www.google.com/search?q={q}"
ddg = "https://duckduckgo.com/?q={q}"
gh = "https://github.com/search?q={q}"
rs = "https://docs.rs/releases/search?query={q}"
crates = "https://crates.io/search?q={q}"
wiki = "https://en.wikipedia.org/w/index.php?search={q}"
so = "https://stackoverflow.com/search?q={q}"
r = "https://www.reddit.com/search/?q={q}"
maps = "https://www.google.com/maps/search/{q}"
"#;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub bangs: HashMap<String, String>,
}

impl AppConfig {
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Self {
        let path_ref = path.as_ref();

        if !path_ref.exists() {
            let _ = fs::write(path_ref, DEFAULT_CONFIG_TOML);
        }

        let raw_content = fs::read_to_string(path_ref)
            .unwrap_or_else(|_| DEFAULT_CONFIG_TOML.to_string());

        let mut parsed: AppConfig = toml::from_str(&raw_content).unwrap_or_else(|_| {
            toml::from_str(DEFAULT_CONFIG_TOML).unwrap_or_default()
        });

        let normalized_bangs = parsed
            .bangs
            .drain()
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();

        AppConfig {
            bangs: normalized_bangs,
        }
    }
}
