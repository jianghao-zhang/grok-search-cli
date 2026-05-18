use std::{collections::BTreeMap, sync::OnceLock};

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Source {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub provider: String,
    pub source_type: SourceType,
    pub provenance: Vec<String>,
    pub observed_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    X,
    Web,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedResponse {
    pub answer: String,
    pub effective_model: Option<String>,
    pub sources: Vec<Source>,
    pub tool_calls: Vec<ToolCall>,
    pub output_types: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn parse_response(raw: &Value, observed_at: DateTime<Utc>) -> ParsedResponse {
    let answer = extract_answer(raw);
    let effective_model = raw
        .get("model")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let tool_calls = extract_tool_calls(raw);
    let output_types = raw
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.get("type").and_then(Value::as_str).map(ToOwned::to_owned))
        .collect::<Vec<_>>();

    let mut candidates: Vec<(String, Option<String>, Option<String>, String)> = Vec::new();

    if let Some(citations) = raw.get("citations").and_then(Value::as_array) {
        for item in citations {
            collect_source_candidate(item, "citation", &mut candidates);
        }
    }

    if let Some(output) = raw.get("output").and_then(Value::as_array) {
        for item in output {
            if let Some(content) = item.get("content").and_then(Value::as_array) {
                for part in content {
                    if let Some(annotations) = part.get("annotations").and_then(Value::as_array) {
                        for annotation in annotations {
                            collect_source_candidate(annotation, "annotation", &mut candidates);
                        }
                    }
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        for url in extract_urls(text) {
                            candidates.push((url, None, None, "output_text".to_string()));
                        }
                    }
                }
            }
        }
    }

    for url in extract_urls(&answer) {
        candidates.push((url, None, None, "answer_text".to_string()));
    }

    let sources = normalize_sources(candidates, observed_at);
    let mut warnings = Vec::new();
    if sources.is_empty() {
        warnings.push("no_sources_detected".to_string());
    }
    if tool_calls
        .iter()
        .any(|t| t.name.starts_with("x_") || t.name == "x_search")
        && !sources.iter().any(|s| s.source_type == SourceType::X)
    {
        warnings.push("x_search_tool_call_without_x_source_url".to_string());
    }

    ParsedResponse {
        answer,
        effective_model,
        sources,
        tool_calls,
        output_types,
        warnings,
    }
}

pub fn extract_urls(text: &str) -> Vec<String> {
    static URL_RE: OnceLock<Regex> = OnceLock::new();
    let re =
        URL_RE.get_or_init(|| Regex::new(r#"https?://[^\s<>"'`，。、；：！？》）】\)]+"#).unwrap());
    let mut seen = Vec::new();
    for m in re.find_iter(text) {
        let url = m.as_str().trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if !seen.iter().any(|u: &String| u == url) {
            seen.push(url.to_string());
        }
    }
    seen
}

fn extract_answer(raw: &Value) -> String {
    let mut parts = Vec::new();

    if let Some(output) = raw.get("output").and_then(Value::as_array) {
        for item in output {
            if let Some(content) = item.get("content").and_then(Value::as_array) {
                for part in content {
                    if part.get("type").and_then(Value::as_str) == Some("output_text")
                        && let Some(text) = part.get("text").and_then(Value::as_str)
                    {
                        parts.push(text.to_string());
                    }
                }
            }
        }
    }

    if parts.is_empty()
        && let Some(text) = raw.get("text").and_then(Value::as_str)
    {
        parts.push(text.to_string());
    }

    if parts.is_empty()
        && let Some(content) = raw
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
    {
        parts.push(content.to_string());
    }

    parts.join("\n").trim().to_string()
}

fn extract_tool_calls(raw: &Value) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    if let Some(output) = raw.get("output").and_then(Value::as_array) {
        for item in output {
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
            let name = item.get("name").and_then(Value::as_str);
            if (item_type.contains("tool") || name.is_some())
                && let Some(name) = name
            {
                let input = item
                    .get("input")
                    .map(parse_json_string_value)
                    .or_else(|| item.get("arguments").map(parse_json_string_value));
                calls.push(ToolCall {
                    name: name.to_string(),
                    call_id: item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned),
                    input,
                    status: item
                        .get("status")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned),
                });
            }
        }
    }
    calls
}

fn parse_json_string_value(value: &Value) -> Value {
    if let Some(s) = value.as_str() {
        serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.to_string()))
    } else {
        value.clone()
    }
}

fn collect_source_candidate(
    value: &Value,
    provenance: &str,
    out: &mut Vec<(String, Option<String>, Option<String>, String)>,
) {
    if let Some(s) = value.as_str() {
        for url in extract_urls(s) {
            out.push((url, None, None, provenance.to_string()));
        }
        return;
    }

    if let Some(obj) = value.as_object() {
        let title = obj
            .get("title")
            .or_else(|| obj.get("name"))
            .or_else(|| obj.get("label"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let description = obj
            .get("description")
            .or_else(|| obj.get("snippet"))
            .or_else(|| obj.get("content"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        for key in ["url", "href", "link", "source", "uri"] {
            if let Some(url) = obj.get(key).and_then(Value::as_str)
                && (url.starts_with("http://") || url.starts_with("https://"))
            {
                out.push((
                    url.to_string(),
                    title.clone(),
                    description.clone(),
                    provenance.to_string(),
                ));
            }
        }

        let json_text = value.to_string();
        for url in extract_urls(&json_text) {
            out.push((
                url,
                title.clone(),
                description.clone(),
                provenance.to_string(),
            ));
        }
    }
}

pub fn normalize_sources(
    candidates: Vec<(String, Option<String>, Option<String>, String)>,
    observed_at: DateTime<Utc>,
) -> Vec<Source> {
    let mut by_url: BTreeMap<String, Source> = BTreeMap::new();
    for (url, title, description, provenance) in candidates {
        let clean = url.trim().trim_end_matches('/').to_string();
        if clean.is_empty() {
            continue;
        }

        let entry = by_url.entry(clean.clone()).or_insert_with(|| {
            let (handle, post_id) = parse_x_url(&clean);
            Source {
                url: clean.clone(),
                title: None,
                description: None,
                provider: if is_x_url(&clean) { "x" } else { "grok" }.to_string(),
                source_type: if is_x_url(&clean) {
                    SourceType::X
                } else if clean.starts_with("http://") || clean.starts_with("https://") {
                    SourceType::Web
                } else {
                    SourceType::Unknown
                },
                provenance: Vec::new(),
                observed_at,
                handle,
                post_id,
            }
        });

        if entry.title.is_none() {
            entry.title = title;
        }
        if entry.description.is_none() {
            entry.description = description;
        }
        if !entry.provenance.iter().any(|p| p == &provenance) {
            entry.provenance.push(provenance);
        }
    }
    by_url.into_values().collect()
}

fn is_x_url(url: &str) -> bool {
    url.contains("://x.com/") || url.contains("://twitter.com/")
}

fn parse_x_url(url: &str) -> (Option<String>, Option<String>) {
    static X_RE: OnceLock<Regex> = OnceLock::new();
    let re = X_RE.get_or_init(|| {
        Regex::new(r#"https?://(?:x|twitter)\.com/([^/\s]+)/status/([0-9]+)"#).unwrap()
    });
    if let Some(caps) = re.captures(url) {
        return (
            caps.get(1).map(|m| m.as_str().to_string()),
            caps.get(2).map(|m| m.as_str().to_string()),
        );
    }
    (None, None)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::{SourceType, extract_urls, parse_response};

    #[test]
    fn extracts_urls_without_trailing_punctuation() {
        assert_eq!(
            extract_urls("see https://x.com/xai/status/123."),
            vec!["https://x.com/xai/status/123"]
        );
    }

    #[test]
    fn parses_x_tool_call_and_annotation() {
        let raw = json!({
            "model": "grok-4.20-multi-agent-0309",
            "output": [
                {
                    "type": "custom_tool_call",
                    "name": "x_keyword_search",
                    "call_id": "xs_0",
                    "input": "{\"query\":\"from:xai Grok\"}",
                    "status": "completed"
                },
                {
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": "https://x.com/xai/status/2055375676656783733",
                        "annotations": [{"url": "https://x.com/xai/status/2055375676656783733", "title": "xAI post"}]
                    }]
                }
            ]
        });
        let parsed = parse_response(&raw, Utc::now());
        assert_eq!(parsed.tool_calls[0].name, "x_keyword_search");
        assert_eq!(parsed.sources[0].source_type, SourceType::X);
        assert_eq!(parsed.sources[0].handle.as_deref(), Some("xai"));
    }
}
