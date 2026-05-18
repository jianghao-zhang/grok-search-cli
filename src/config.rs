use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

pub const OFFICIAL_XAI_API_URL: &str = "https://api.x.ai/v1";
pub const OFFICIAL_XAI_EU_API_URL: &str = "https://eu-west-1.api.x.ai/v1";
pub const DEFAULT_OFFICIAL_MODEL: &str = "grok-4.3";
pub const DEFAULT_PROXY_MODEL: &str = "grok-4.20-multi-agent-console";

#[derive(Debug, Clone)]
pub struct Config {
    pub config_file: PathBuf,
    pub grok_provider: GrokProvider,
    pub grok_api_url: Option<String>,
    pub grok_api_key: Option<String>,
    pub grok_model: String,
    pub tavily_api_url: String,
    pub tavily_api_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MaskedConfig {
    pub config_file: String,
    pub grok_provider: String,
    pub grok_api_url: String,
    pub grok_api_key: String,
    pub grok_model: String,
    pub tavily_api_url: String,
    pub tavily_api_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrokProvider {
    XaiOfficial,
    Grok2ApiProxy,
    CustomOpenAiCompatible,
}

impl GrokProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::XaiOfficial => "xai-official",
            Self::Grok2ApiProxy => "grok2api-proxy",
            Self::CustomOpenAiCompatible => "custom-openai-compatible",
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_file = config_file();
        let file = read_config_file(&config_file);

        let explicit_provider =
            string_field(&file, "provider").and_then(|value| parse_provider(&value));
        let xai_key_from_env = env::var("XAI_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_some();

        let grok_api_url = first_nonempty(&[
            env::var("GROK_API_URL").ok(),
            env::var("XAI_API_URL").ok(),
            env::var("XAI_BASE_URL").ok(),
            string_field(&file, "api_url"),
        ]);

        let grok_api_key = first_nonempty(&[
            env::var("GROK_API_KEY").ok(),
            env::var("XAI_API_KEY").ok(),
            string_field(&file, "api_key"),
        ]);

        let grok_provider =
            infer_provider(explicit_provider, grok_api_url.as_deref(), xai_key_from_env);
        let grok_api_url = grok_api_url.or_else(|| match grok_provider {
            GrokProvider::XaiOfficial => Some(OFFICIAL_XAI_API_URL.to_string()),
            _ => None,
        });
        let default_model = match grok_provider {
            GrokProvider::Grok2ApiProxy => DEFAULT_PROXY_MODEL,
            GrokProvider::XaiOfficial | GrokProvider::CustomOpenAiCompatible => {
                DEFAULT_OFFICIAL_MODEL
            }
        };
        let grok_model = first_nonempty(&[
            env::var("GROK_MODEL").ok(),
            env::var("XAI_MODEL").ok(),
            string_field(&file, "model"),
            Some(default_model.to_string()),
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
            grok_provider,
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
            grok_provider: self.grok_provider.as_str().to_string(),
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
    set_config_field("model", model)
}

pub fn set_api_url(api_url: &str) -> Result<()> {
    let path = config_file();
    let mut value = read_config_file(&path);
    if !value.is_object() {
        value = json!({});
    }
    value["api_url"] = Value::String(api_url.to_string());
    value["provider"] = Value::String(
        infer_provider(None, Some(api_url), false)
            .as_str()
            .to_string(),
    );
    write_config_file(&path, &value)
}

pub fn set_api_key(api_key: &str) -> Result<()> {
    set_config_field("api_key", api_key)
}

pub fn set_tavily_key(key: &str) -> Result<()> {
    set_config_field("tavily_api_key", key)
}

pub fn use_official(api_key: Option<&str>, api_url: &str, model: &str) -> Result<()> {
    let path = config_file();
    let mut value = read_config_file(&path);
    if !value.is_object() {
        value = json!({});
    }
    value["provider"] = Value::String(GrokProvider::XaiOfficial.as_str().to_string());
    value["api_url"] = Value::String(api_url.to_string());
    value["model"] = Value::String(model.to_string());
    if let Some(api_key) = api_key {
        value["api_key"] = Value::String(api_key.to_string());
    }
    write_config_file(&path, &value)
}

pub fn use_proxy(api_key: Option<&str>, api_url: &str, model: &str) -> Result<()> {
    let path = config_file();
    let mut value = read_config_file(&path);
    if !value.is_object() {
        value = json!({});
    }
    value["provider"] = Value::String(GrokProvider::Grok2ApiProxy.as_str().to_string());
    value["api_url"] = Value::String(api_url.to_string());
    value["model"] = Value::String(model.to_string());
    if let Some(api_key) = api_key {
        value["api_key"] = Value::String(api_key.to_string());
    }
    write_config_file(&path, &value)
}

fn set_config_field(field: &str, value_str: &str) -> Result<()> {
    let path = config_file();
    let mut value = read_config_file(&path);
    if !value.is_object() {
        value = json!({});
    }
    value[field] = Value::String(value_str.to_string());
    write_config_file(&path, &value)
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

fn write_config_file(path: &PathBuf, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
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

fn parse_provider(value: &str) -> Option<GrokProvider> {
    match value.trim().to_ascii_lowercase().as_str() {
        "xai" | "xai-official" | "official" => Some(GrokProvider::XaiOfficial),
        "grok2api" | "grok2api-proxy" | "proxy" | "local-proxy" => {
            Some(GrokProvider::Grok2ApiProxy)
        }
        "custom" | "openai-compatible" | "custom-openai-compatible" => {
            Some(GrokProvider::CustomOpenAiCompatible)
        }
        _ => None,
    }
}

fn infer_provider(
    explicit: Option<GrokProvider>,
    api_url: Option<&str>,
    xai_key_from_env: bool,
) -> GrokProvider {
    if let Some(provider) = explicit {
        return provider;
    }
    let Some(api_url) = api_url else {
        return if xai_key_from_env {
            GrokProvider::XaiOfficial
        } else {
            GrokProvider::CustomOpenAiCompatible
        };
    };
    let normalized = api_url.trim().to_ascii_lowercase();
    if normalized.contains("api.x.ai") {
        GrokProvider::XaiOfficial
    } else if normalized.contains("127.0.0.1")
        || normalized.contains("localhost")
        || normalized.contains("grok2api")
    {
        GrokProvider::Grok2ApiProxy
    } else {
        GrokProvider::CustomOpenAiCompatible
    }
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
    use super::{GrokProvider, infer_provider, mask};

    #[test]
    fn masks_long_secret() {
        assert_eq!(mask("123456789abc"), "1234****9abc");
    }

    #[test]
    fn infers_official_provider_from_xai_url() {
        assert_eq!(
            infer_provider(None, Some("https://api.x.ai/v1"), false),
            GrokProvider::XaiOfficial
        );
    }

    #[test]
    fn infers_proxy_provider_from_local_url() {
        assert_eq!(
            infer_provider(None, Some("http://127.0.0.1:8000/v1"), false),
            GrokProvider::Grok2ApiProxy
        );
    }

    #[test]
    fn xai_key_without_url_defaults_to_official() {
        assert_eq!(infer_provider(None, None, true), GrokProvider::XaiOfficial);
    }
}
