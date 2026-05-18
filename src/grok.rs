use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{Duration as ChronoDuration, Local, Utc};
use reqwest::Client;
use serde_json::{Value, json};

use crate::{
    artifact::{self, SearchArtifact, ToolSummary},
    cli::SearchArgs,
    config::Config,
    source::{self, Source},
    web_tools,
};

const SEARCH_SYSTEM_PROMPT: &str = r#"You are Grok Search CLI's realtime source reconnaissance engine.
Find current, source-grade evidence. Prefer primary sources, exact URLs, and recent timestamps.
When X Search is available, use it for X-native community evidence rather than generic web guesses.
Return concise answer text and preserve citations in output annotations whenever possible."#;

pub async fn search(config: &Config, mut args: SearchArgs) -> Result<SearchArtifact> {
    let query = args.query_text();
    let model = args
        .model
        .clone()
        .unwrap_or_else(|| config.grok_model.clone());
    let api_url = config
        .grok_api_url
        .as_deref()
        .context("GROK_API_URL is not configured")?;
    let api_key = config
        .grok_api_key
        .as_deref()
        .context("GROK_API_KEY is not configured")?;
    apply_recency_window(&mut args);

    let tools = build_tools(&args);
    let mut user_content = format!("{}\n\n{}", current_time_context(), query);
    if let Some(platform) = &args.platform {
        user_content.push_str(&format!("\n\nPlatform focus: {platform}"));
    }
    if args.prefer_recent {
        user_content.push_str("\n\nTemporal evidence weighting: prefer newer evidence when credible sources conflict, but keep conflicts visible.");
    }
    user_content.push_str(&format!("\n\nConflict policy: {}.", args.conflict_policy));

    let mut body = json!({
        "model": model,
        "input": [
            {"role": "system", "content": SEARCH_SYSTEM_PROMPT},
            {"role": "user", "content": user_content}
        ],
        "tools": tools,
        "stream": false
    });
    if let Some(max) = args.max_output_tokens {
        body["max_output_tokens"] = json!(max);
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(args.timeout_seconds))
        .build()?;
    let response = client
        .post(format!("{}/responses", api_url.trim_end_matches('/')))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .context("send Grok /responses request")?;
    let status = response.status();
    let raw_text = response.text().await?;
    if !status.is_success() {
        anyhow::bail!("Grok /responses returned HTTP {}: {}", status, raw_text);
    }
    let raw: Value = serde_json::from_str(&raw_text).context("parse Grok /responses JSON")?;
    let parsed = source::parse_response(&raw, Utc::now());

    let mut sources = parsed.sources.clone();
    if args.extra_sources > 0 {
        let extra = web_tools::supplemental_search(config, &query, args.extra_sources).await;
        match extra {
            Ok(mut extra_sources) => merge_sources(&mut sources, &mut extra_sources),
            Err(err) => {
                let mut warnings = parsed.warnings.clone();
                warnings.push(format!("extra_sources_failed: {err}"));
                return save_artifact(
                    args,
                    ArtifactParts {
                        query,
                        model_requested: model,
                        parsed,
                        sources,
                        raw,
                        warnings,
                    },
                );
            }
        }
    }

    let warnings = parsed.warnings.clone();
    save_artifact(
        args,
        ArtifactParts {
            query,
            model_requested: model,
            parsed,
            sources,
            raw,
            warnings,
        },
    )
}

pub async fn models(config: &Config, timeout_seconds: u64) -> Result<Vec<String>> {
    let api_url = config
        .grok_api_url
        .as_deref()
        .context("GROK_API_URL is not configured")?;
    let api_key = config
        .grok_api_key
        .as_deref()
        .context("GROK_API_KEY is not configured")?;
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let response = client
        .get(format!("{}/models", api_url.trim_end_matches('/')))
        .bearer_auth(api_key)
        .send()
        .await
        .context("send /models request")?;
    let status = response.status();
    let raw_text = response.text().await?;
    if !status.is_success() {
        anyhow::bail!("/models returned HTTP {}: {}", status, raw_text);
    }
    let raw: Value = serde_json::from_str(&raw_text).context("parse /models JSON")?;
    let models = raw
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get("id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect();
    Ok(models)
}

struct ArtifactParts {
    query: String,
    model_requested: String,
    parsed: source::ParsedResponse,
    sources: Vec<Source>,
    raw: Value,
    warnings: Vec<String>,
}

fn save_artifact(args: SearchArgs, parts: ArtifactParts) -> Result<SearchArtifact> {
    let tool_summary = ToolSummary {
        web_search: !args.no_web_search,
        x_search: args.x_search,
        allowed_domains: args.allowed_domain.clone(),
        excluded_domains: args.excluded_domain.clone(),
        allowed_x_handles: args.allowed_x_handle.clone(),
        excluded_x_handles: args.excluded_x_handle.clone(),
        from_date: args.from_date.clone(),
        to_date: args.to_date.clone(),
        prefer_recent: args.prefer_recent,
        conflict_policy: args.conflict_policy.to_string(),
    };

    let artifact = SearchArtifact {
        id: SearchArtifact::fresh_id(),
        created_at: Utc::now(),
        query: parts.query,
        model_requested: parts.model_requested,
        model_effective: parts.parsed.effective_model,
        tool_summary,
        answer: parts.parsed.answer,
        sources: parts.sources,
        tool_calls: parts.parsed.tool_calls,
        warnings: parts.warnings,
        raw_response: args.include_raw.then_some(parts.raw),
        artifact_path: None,
        format: args.format,
    };
    artifact::save(artifact, args.artifact_dir.as_deref())
}

fn build_tools(args: &SearchArgs) -> Vec<Value> {
    let mut tools = Vec::new();
    if !args.no_web_search {
        let mut web = json!({"type": "web_search"});
        if !args.allowed_domain.is_empty() {
            web["allowed_domains"] = json!(args.allowed_domain.iter().take(5).collect::<Vec<_>>());
        }
        if !args.excluded_domain.is_empty() {
            web["excluded_domains"] =
                json!(args.excluded_domain.iter().take(5).collect::<Vec<_>>());
        }
        if args.enable_image_understanding {
            web["enable_image_understanding"] = json!(true);
        }
        tools.push(web);
    }

    if args.x_search {
        let mut x = json!({"type": "x_search"});
        if !args.allowed_x_handle.is_empty() {
            x["allowed_x_handles"] = json!(normalize_handles(&args.allowed_x_handle));
        }
        if !args.excluded_x_handle.is_empty() {
            x["excluded_x_handles"] = json!(normalize_handles(&args.excluded_x_handle));
        }
        if let Some(from) = &args.from_date {
            x["from_date"] = json!(from);
        }
        if let Some(to) = &args.to_date {
            x["to_date"] = json!(to);
        }
        if args.enable_image_understanding {
            x["enable_image_understanding"] = json!(true);
        }
        if args.enable_video_understanding {
            x["enable_video_understanding"] = json!(true);
        }
        tools.push(x);
    }

    tools
}

fn normalize_handles(handles: &[String]) -> Vec<String> {
    handles
        .iter()
        .take(10)
        .map(|h| h.trim().trim_start_matches('@').to_string())
        .filter(|h| !h.is_empty())
        .collect()
}

fn apply_recency_window(args: &mut SearchArgs) {
    if args.from_date.is_some() {
        return;
    }
    if let Some(days) = args.recency_days {
        let date = Local::now().date_naive() - ChronoDuration::days(days);
        args.from_date = Some(date.format("%Y-%m-%d").to_string());
    }
}

fn current_time_context() -> String {
    let now = Local::now();
    format!(
        "[Current Time Context]\n- Date: {}\n- Time: {}\n- Timezone: {}",
        now.format("%Y-%m-%d"),
        now.format("%H:%M:%S"),
        now.offset()
    )
}

fn merge_sources(existing: &mut Vec<Source>, extra: &mut Vec<Source>) {
    for source in extra.drain(..) {
        if !existing.iter().any(|s| s.url == source.url) {
            existing.push(source);
        }
    }
}
