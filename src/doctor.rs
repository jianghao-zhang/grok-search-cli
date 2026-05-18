use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::{Duration as ChronoDuration, Utc};
use reqwest::Client;
use serde_json::{Value, json};
use tokio::{process::Command, time::timeout};

use crate::{cli::DoctorArgs, config::Config, source};

pub async fn run(config: &Config, args: &DoctorArgs) -> Result<Value> {
    let bird = check_bird(args.timeout_seconds).await;
    let mut report = json!({
        "config": config.masked(),
        "checks": {
            "grok_configured": config.grok_api_url.is_some() && config.grok_api_key.is_some(),
            "model_configured": !config.grok_model.trim().is_empty(),
            "tavily_configured": config.tavily_api_key.is_some(),
            "bird_available": bird.get("available").and_then(Value::as_bool).unwrap_or(false),
            "bird_credentials_ok": bird.get("credentials_ok").and_then(Value::as_bool).unwrap_or(false)
        }
    });
    report["bird"] = bird;

    if config.grok_api_url.is_some() && config.grok_api_key.is_some() {
        report["models"] = check_models(config, args.timeout_seconds).await;
    }

    if args.live_x {
        report["x_search"] = live_x_smoke(config, args.timeout_seconds).await;
    }

    Ok(report)
}

async fn check_bird(timeout_seconds: u64) -> Value {
    let command_timeout = Duration::from_secs(timeout_seconds.clamp(1, 10));
    let version = timeout(
        command_timeout,
        Command::new("bird").arg("--version").output(),
    )
    .await;
    let Ok(Ok(version_output)) = version else {
        return json!({
            "available": false,
            "credentials_ok": false,
            "hint": "Install bird CLI to fetch X sources locally, or use source index/links without fetching X bodies."
        });
    };
    if !version_output.status.success() {
        return json!({
            "available": false,
            "credentials_ok": false,
            "hint": "bird command exists but did not run successfully."
        });
    }

    let check = timeout(
        command_timeout,
        Command::new("bird").arg("check").arg("--no-color").output(),
    )
    .await;
    let credentials_ok = matches!(check, Ok(Ok(output)) if output.status.success());

    json!({
        "available": true,
        "credentials_ok": credentials_ok,
        "version": String::from_utf8_lossy(&version_output.stdout).trim(),
        "hint": if credentials_ok {
            "X source fetch is available through bird read --json."
        } else {
            "bird is installed, but credentials are not ready. source index/links still work; use --no-x for full batch fetch."
        }
    })
}

async fn check_models(config: &Config, timeout_seconds: u64) -> Value {
    let Some(api_url) = config.grok_api_url.as_deref() else {
        return json!({"ok": false, "error": "GROK_API_URL is not configured"});
    };
    let Some(api_key) = config.grok_api_key.as_deref() else {
        return json!({"ok": false, "error": "GROK_API_KEY is not configured"});
    };

    let start = Instant::now();
    let result = async {
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
        let text = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("HTTP {}: {}", status, text);
        }
        let data: Value = serde_json::from_str(&text)?;
        let models = data
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let ids: Vec<String> = models
            .iter()
            .filter_map(|item| {
                item.get("id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .collect();
        Ok::<_, anyhow::Error>(ids)
    }
    .await;

    match result {
        Ok(ids) => json!({
            "ok": true,
            "latency_ms": start.elapsed().as_millis(),
            "count": ids.len(),
            "current_model": config.grok_model,
            "current_model_available": ids.iter().any(|id| id == &config.grok_model),
            "interesting_models": ids.into_iter().filter(|id| {
                id == "grok-4.20-multi-agent-console"
                    || id == "grok-4.3"
                    || id == "grok-4.20"
                    || id == "grok-4.20-reasoning"
                    || id == "grok-4-1-fast-reasoning"
                    || id == "grok-4-fast-reasoning"
            }).collect::<Vec<_>>()
        }),
        Err(err) => json!({
            "ok": false,
            "latency_ms": start.elapsed().as_millis(),
            "error": err.to_string()
        }),
    }
}

async fn live_x_smoke(config: &Config, timeout_seconds: u64) -> Value {
    let Some(api_url) = config.grok_api_url.as_deref() else {
        return json!({"ok": false, "error": "GROK_API_URL is not configured"});
    };
    let Some(api_key) = config.grok_api_key.as_deref() else {
        return json!({"ok": false, "error": "GROK_API_KEY is not configured"});
    };

    let from_date = (Utc::now().date_naive() - ChronoDuration::days(21))
        .format("%Y-%m-%d")
        .to_string();
    let start = Instant::now();
    let result = async {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()?;
        let body = json!({
            "model": config.grok_model,
            "input": [{
                "role": "user",
                "content": "Use X Search to find one recent public @xai post mentioning Grok. Return the URL only."
            }],
            "tools": [{
                "type": "x_search",
                "allowed_x_handles": ["xai"],
                "from_date": from_date
            }],
            "stream": false
        });
        let response = client
            .post(format!("{}/responses", api_url.trim_end_matches('/')))
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .context("send /responses x_search smoke request")?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("HTTP {}: {}", status, text);
        }
        let raw: Value = serde_json::from_str(&text)?;
        Ok::<_, anyhow::Error>(source::parse_response(&raw, Utc::now()))
    }
    .await;

    match result {
        Ok(parsed) => json!({
            "ok": parsed.tool_calls.iter().any(|call| call.name.starts_with("x_")),
            "latency_ms": start.elapsed().as_millis(),
            "tool_calls": parsed.tool_calls,
            "x_sources_count": parsed.sources.iter().filter(|source| source.source_type == source::SourceType::X).count(),
            "sample_x_urls": parsed.sources.into_iter().filter(|source| source.source_type == source::SourceType::X).take(3).map(|source| source.url).collect::<Vec<_>>(),
            "warnings": parsed.warnings
        }),
        Err(err) => json!({
            "ok": false,
            "latency_ms": start.elapsed().as_millis(),
            "error": err.to_string()
        }),
    }
}
