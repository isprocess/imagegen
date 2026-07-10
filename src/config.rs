use crate::provider::ProviderKind;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub default_model: Option<String>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

impl Config {
    pub fn merge_env(mut file: Config, env: &BTreeMap<String, String>) -> Config {
        if let Some(value) = nonempty(env.get("IMAGEGEN_DEFAULT_MODEL")) {
            file.default_model = Some(value.to_string());
        }
        apply_provider_env(
            &mut file,
            "openai",
            env.get("IMAGEGEN_OPENAI_BASE_URL"),
            env.get("IMAGEGEN_OPENAI_API_KEY"),
        );
        apply_provider_env(
            &mut file,
            "google",
            env.get("IMAGEGEN_GOOGLE_BASE_URL"),
            env.get("IMAGEGEN_GOOGLE_API_KEY"),
        );
        file
    }

    pub fn provider(&self, kind: ProviderKind) -> ProviderConfig {
        self.providers
            .get(kind.key())
            .cloned()
            .unwrap_or_else(ProviderConfig::default)
    }
}

fn apply_provider_env(
    config: &mut Config,
    provider: &str,
    base_url: Option<&String>,
    api_key: Option<&String>,
) {
    let entry = config.providers.entry(provider.to_string()).or_default();
    if let Some(value) = nonempty(base_url) {
        entry.base_url = Some(value.to_string());
    }
    if let Some(value) = nonempty(api_key) {
        entry.api_key = Some(value.to_string());
    }
}

fn nonempty(value: Option<&String>) -> Option<&str> {
    let value = value?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

pub fn load_config(path: Option<&Path>) -> Result<Config> {
    let path = path.map(PathBuf::from).unwrap_or_else(default_config_path);
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("invalid config JSON {}", path.display()))
}

pub fn collect_env() -> BTreeMap<String, String> {
    std::env::vars()
        .filter(|(key, _)| key.starts_with("IMAGEGEN_"))
        .collect()
}

pub fn load_effective_config(path: Option<&Path>) -> Result<Config> {
    let config_path = explicit_or_env_path(path);
    let file = load_config(config_path.as_deref())?;
    Ok(Config::merge_env(file, &collect_env()))
}

fn explicit_or_env_path(path: Option<&Path>) -> Option<PathBuf> {
    path.map(PathBuf::from).or_else(|| {
        std::env::var("IMAGEGEN_CONFIG")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
    })
}

pub fn default_config_path() -> PathBuf {
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("imagegen").join("config.json");
        }
        return home_dir()
            .join("AppData")
            .join("Roaming")
            .join("imagegen")
            .join("config.json");
    }
    home_dir()
        .join(".config")
        .join("imagegen")
        .join("config.json")
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn write_provider_config(
    path: Option<&Path>,
    provider: ProviderKind,
    base_url: &str,
    api_key: &str,
) -> Result<PathBuf> {
    validate_config_value("baseUrl", base_url)?;
    validate_config_value("apiKey", api_key)?;
    let path = path.map(PathBuf::from).unwrap_or_else(default_config_path);
    let mut config = load_config(Some(&path)).unwrap_or_default();
    config.providers.insert(
        provider.key().to_string(),
        ProviderConfig {
            base_url: Some(base_url.trim().to_string()),
            api_key: Some(api_key.trim().to_string()),
        },
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&config)? + "\n";
    fs::write(&path, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

pub fn validate_config_value(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{label} is required");
    }
    if label == "apiKey" && value.trim() == "YOUR_API_KEY" {
        bail!("apiKey must be replaced with a real key");
    }
    Ok(())
}
