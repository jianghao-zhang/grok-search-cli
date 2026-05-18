use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

const DEFAULT_MODEL: &str = "grok-4.20-multi-agent-console";

#[derive(Debug, Clone)]
pub struct Config {
    pub config_file: PathBuf,
    pub grok_api_url: Option<String>,
    pub grok_api_key: Option<String>,
    pub grok_model: String,
    pub tavily_api_url: String,
    pub tavily_api_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MaskedConfig {
    pub config_file: String,
    pub grok_api_url: String,
    pub grok_api_key: String,
    pub grok_model: String,
    pub tavily_api_url: String,
    pub tavily_api_key: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_file = config_file();
        let file = read_config_file(&config_file);

        let grok_api_url = first_nonempty(&[
            env::var("GROK_API_URL").ok(),
            string_field(&file, "api_url"),
        ]);

        let grok_api_key = first_nonempty(&[
            env::var("GROK_API_KEY").ok(),
            string_field(&file, "api_key"),
        ]);

        let grok_model = first_nonempty(&[
            env::var("GROK_MODEL").ok(),
            string_field(&file, "model"),
            Some(DEFAULT_MODEL.to_string()),
        ])
        .unwrap();

        let tavily_api_url = first_nonempty(&[
            env::var("TAVILY_API_URL").ok(),
            Some("https://api.tavily.com".to_string()),
        ])
        .unwrap();

        let tavily_api_key = first_nonempty(&[
            env::var("TAVILY_API_KEY").ok(),
            string_field(&file, "tavily_api_key"),
        ]);

        Ok(Self {
            config_file,
            grok_api_url,
            grok_api_key,
            grok_model,
            tavily_api_url,
            tavily_api_key,
        })
    }

    pub fn masked(&self) -> MaskedConfig {
        MaskedConfig {
            config_file: self.config_file.display().to_string(),
            grok_api_url: self
                .grok_api_url
                .clone()
                .unwrap_or_else(|| "not configured".to_string()),
            grok_api_key: self
                .grok_api_key
                .as_deref()
                .map(mask)
                .unwrap_or_else(|| "not configured".to_string()),
            grok_model: self.grok_model.clone(),
            tavily_api_url: self.tavily_api_url.clone(),
            tavily_api_key: self
                .tavily_api_key
                .as_deref()
                .map(mask)
                .unwrap_or_else(|| "not configured".to_string()),
        }
    }
}

pub fn set_model(model: &str) -> Result<()> {
    let path = config_file();
    let mut value = read_config_file(&path);
    if !value.is_object() {
        value = json!({});
    }
    value["model"] = Value::String(model.to_string());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&value)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn config_file() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("grok-search")
        .join("config.json")
}

fn read_config_file(path: &PathBuf) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn first_nonempty(values: &[Option<String>]) -> Option<String> {
    values
        .iter()
        .flatten()
        .find(|s| !s.trim().is_empty())
        .cloned()
}

fn mask(secret: &str) -> String {
    let len = secret.chars().count();
    if len <= 8 {
        return "***".to_string();
    }
    let head: String = secret.chars().take(4).collect();
    let tail: String = secret
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}{}{}", head, "*".repeat(len.saturating_sub(8)), tail)
}

pub fn require_key<'a>(key: &'a Option<String>, name: &str) -> Result<&'a str> {
    match key.as_deref() {
        Some(value) if !value.is_empty() => Ok(value),
        _ => bail!("{name} is not configured"),
    }
}

#[cfg(test)]
mod tests {
    use super::mask;

    #[test]
    fn masks_long_secret() {
        assert_eq!(mask("123456789abc"), "1234****9abc");
    }
}
