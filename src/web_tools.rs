use std::{fs, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::task::JoinSet;

use crate::{
    artifact::SearchArtifact,
    cli::{FetchArgs, FetchSourcesArgs, MapArgs},
    config::{Config, require_key},
    source::{self, Source, SourceType},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchResult {
    pub url: String,
    pub provider: String,
    pub markdown: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchSourcesResult {
    pub session_id: String,
    pub output_dir: String,
    pub source_count: usize,
    pub chunk_count: usize,
    pub ok_chunks: usize,
    pub failed_chunks: usize,
    pub chunks: Vec<FetchSourcesChunk>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchSourcesChunk {
    pub index: usize,
    pub url_count: usize,
    pub output_file: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn fetch(config: &Config, args: &FetchArgs) -> Result<FetchResult> {
    let markdown = tavily_extract(config, &args.url, args.timeout_seconds).await?;
    Ok(FetchResult {
        url: args.url.clone(),
        provider: "tavily".to_string(),
        markdown,
    })
}

pub async fn fetch_sources(
    config: &Config,
    artifact: &SearchArtifact,
    args: &FetchSourcesArgs,
) -> Result<FetchSourcesResult> {
    let api_key = require_key(&config.tavily_api_key, "TAVILY_API_KEY")?.to_string();
    let api_url = config.tavily_api_url.trim_end_matches('/').to_string();
    let output_dir = args
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("tavily-{}", artifact.id)));
    fs::create_dir_all(&output_dir).with_context(|| format!("create {}", output_dir.display()))?;

    let urls: Vec<String> = artifact
        .sources
        .iter()
        .filter(|source| args.include_x || source.source_type != SourceType::X)
        .map(|source| source.url.clone())
        .collect();

    if urls.is_empty() {
        anyhow::bail!("no Tavily-fetchable sources found; use --include-x to include X URLs");
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(args.timeout_seconds))
        .build()?;
    let chunk_size = args.chunk_size.max(1);
    let chunks: Vec<Vec<String>> = urls
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect();
    let chunk_count = chunks.len();
    let mut next = 0usize;
    let mut set = JoinSet::new();
    let parallel = args.parallel.max(1);
    let mut results = Vec::new();

    while next < chunk_count || !set.is_empty() {
        while next < chunk_count && set.len() < parallel {
            let chunk_urls = chunks[next].clone();
            let client = client.clone();
            let api_key = api_key.clone();
            let api_url = api_url.clone();
            let output_file = output_dir.join(format!("chunk-{next:03}.json"));
            let index = next;
            set.spawn(async move {
                fetch_source_chunk(client, api_url, api_key, output_file, index, chunk_urls).await
            });
            next += 1;
        }

        if let Some(joined) = set.join_next().await {
            results.push(match joined {
                Ok(chunk) => chunk,
                Err(err) => FetchSourcesChunk {
                    index: usize::MAX,
                    url_count: 0,
                    output_file: String::new(),
                    ok: false,
                    error: Some(err.to_string()),
                },
            });
        }
    }

    results.sort_by_key(|chunk| chunk.index);
    let ok_chunks = results.iter().filter(|chunk| chunk.ok).count();
    let failed_chunks = results.len().saturating_sub(ok_chunks);
    let result = FetchSourcesResult {
        session_id: artifact.id.clone(),
        output_dir: output_dir.display().to_string(),
        source_count: urls.len(),
        chunk_count,
        ok_chunks,
        failed_chunks,
        chunks: results,
    };

    let manifest_path = output_dir.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&result)?)
        .with_context(|| format!("write {}", manifest_path.display()))?;
    Ok(result)
}

async fn fetch_source_chunk(
    client: Client,
    api_url: String,
    api_key: String,
    output_file: PathBuf,
    index: usize,
    urls: Vec<String>,
) -> FetchSourcesChunk {
    let url_count = urls.len();
    let output_file_string = output_file.display().to_string();
    let response = client
        .post(format!("{api_url}/extract"))
        .bearer_auth(api_key)
        .json(&json!({"urls": urls, "format": "markdown"}))
        .send()
        .await;

    match response {
        Ok(response) => {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            if status.is_success() {
                match fs::write(&output_file, text) {
                    Ok(()) => FetchSourcesChunk {
                        index,
                        url_count,
                        output_file: output_file_string,
                        ok: true,
                        error: None,
                    },
                    Err(err) => FetchSourcesChunk {
                        index,
                        url_count,
                        output_file: output_file_string,
                        ok: false,
                        error: Some(err.to_string()),
                    },
                }
            } else {
                FetchSourcesChunk {
                    index,
                    url_count,
                    output_file: output_file_string,
                    ok: false,
                    error: Some(format!("HTTP {}: {}", status, text)),
                }
            }
        }
        Err(err) => FetchSourcesChunk {
            index,
            url_count,
            output_file: output_file_string,
            ok: false,
            error: Some(err.to_string()),
        },
    }
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
