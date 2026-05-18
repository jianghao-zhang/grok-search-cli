use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    cli::{FetchArgs, MapArgs},
    config::{Config, require_key},
    source::{self, Source},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchResult {
    pub url: String,
    pub provider: String,
    pub markdown: String,
}

pub async fn fetch(config: &Config, args: &FetchArgs) -> Result<FetchResult> {
    let markdown = tavily_extract(config, &args.url, args.timeout_seconds).await?;
    Ok(FetchResult {
        url: args.url.clone(),
        provider: "tavily".to_string(),
        markdown,
    })
}

pub async fn map(config: &Config, args: &MapArgs) -> Result<Value> {
    let api_key = require_key(&config.tavily_api_key, "TAVILY_API_KEY")?;
    let client = Client::builder()
        .timeout(Duration::from_secs(args.timeout_seconds + 10))
        .build()?;
    let mut body = json!({
        "url": args.url,
        "max_depth": args.max_depth.clamp(1, 5),
        "max_breadth": args.max_breadth.clamp(1, 500),
        "limit": args.limit.clamp(1, 500),
        "timeout": args.timeout_seconds.clamp(10, 150)
    });
    if !args.instructions.trim().is_empty() {
        body["instructions"] = json!(args.instructions);
    }
    let response = client
        .post(format!(
            "{}/map",
            config.tavily_api_url.trim_end_matches('/')
        ))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .context("send Tavily map request")?;
    json_response(response).await
}

pub async fn supplemental_search(
    config: &Config,
    query: &str,
    limit: usize,
) -> Result<Vec<Source>> {
    let mut sources = Vec::new();

    if config.tavily_api_key.is_some()
        && let Ok(mut tavily) = tavily_search(config, query, limit).await
    {
        sources.append(&mut tavily);
    }

    if sources.is_empty() {
        anyhow::bail!("no supplemental source provider returned results");
    }

    Ok(sources)
}

async fn tavily_extract(config: &Config, url: &str, timeout_seconds: u64) -> Result<String> {
    let api_key = require_key(&config.tavily_api_key, "TAVILY_API_KEY")?;
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let response = client
        .post(format!(
            "{}/extract",
            config.tavily_api_url.trim_end_matches('/')
        ))
        .bearer_auth(api_key)
        .json(&json!({"urls": [url], "format": "markdown"}))
        .send()
        .await
        .context("send Tavily extract request")?;
    let value = json_response(response).await?;
    let markdown = value
        .pointer("/results/0/raw_content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if markdown.trim().is_empty() {
        anyhow::bail!("Tavily returned empty content");
    }
    Ok(markdown)
}

async fn tavily_search(config: &Config, query: &str, limit: usize) -> Result<Vec<Source>> {
    let api_key = require_key(&config.tavily_api_key, "TAVILY_API_KEY")?;
    let client = Client::builder().timeout(Duration::from_secs(90)).build()?;
    let response = client
        .post(format!(
            "{}/search",
            config.tavily_api_url.trim_end_matches('/')
        ))
        .bearer_auth(api_key)
        .json(&json!({
            "query": query,
            "max_results": limit,
            "search_depth": "advanced",
            "include_raw_content": false,
            "include_answer": false
        }))
        .send()
        .await
        .context("send Tavily search request")?;
    let value = json_response(response).await?;
    let candidates = value
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let url = item.get("url")?.as_str()?.to_string();
            Some((
                url,
                item.get("title")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                item.get("content")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                "tavily".to_string(),
            ))
        })
        .collect();
    Ok(source::normalize_sources(candidates, Utc::now()))
}

async fn json_response(response: reqwest::Response) -> Result<Value> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        anyhow::bail!("HTTP {}: {}", status, text);
    }
    serde_json::from_str(&text).context("parse JSON response")
}
