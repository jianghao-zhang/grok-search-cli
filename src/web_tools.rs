use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{process::Command, task::JoinSet, time::timeout};

use crate::{
    artifact::SearchArtifact,
    cli::{FetchArgs, FetchSourceArgs, FetchSourcesArgs, MapArgs},
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
    pub web_source_count: usize,
    pub x_source_count: usize,
    pub chunk_count: usize,
    pub ok_chunks: usize,
    pub failed_chunks: usize,
    pub chunks: Vec<FetchSourcesChunk>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchSourcesChunk {
    pub index: usize,
    pub provider: String,
    pub source_type: String,
    pub url_count: usize,
    pub urls: Vec<String>,
    pub output_file: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchedSourceResult {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_index: Option<usize>,
    pub url: String,
    pub provider: String,
    pub source_type: String,
    pub output_file: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct WebSourceBatch {
    client: Client,
    api_url: String,
    api_key: String,
    output_dir: PathBuf,
    urls: Vec<String>,
    chunk_size: usize,
    parallel: usize,
    start_index: usize,
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
    let output_dir = args
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("sources-{}", artifact.id)));
    fs::create_dir_all(&output_dir).with_context(|| format!("create {}", output_dir.display()))?;

    let web_urls: Vec<String> = artifact
        .sources
        .iter()
        .filter(|source| !args.no_web && source.source_type != SourceType::X)
        .map(|source| source.url.clone())
        .collect();
    let x_sources: Vec<Source> = artifact
        .sources
        .iter()
        .filter(|source| !args.no_x && source.source_type == SourceType::X)
        .cloned()
        .collect();

    if web_urls.is_empty() && x_sources.is_empty() {
        anyhow::bail!("no fetchable sources found");
    }

    let mut results = Vec::new();
    if !web_urls.is_empty() {
        let api_key = require_key(&config.tavily_api_key, "TAVILY_API_KEY")?.to_string();
        let client = Client::builder()
            .timeout(Duration::from_secs(args.timeout_seconds))
            .build()?;
        results.extend(
            fetch_web_source_chunks(WebSourceBatch {
                client,
                api_url: config.tavily_api_url.trim_end_matches('/').to_string(),
                api_key,
                output_dir: output_dir.clone(),
                urls: web_urls.clone(),
                chunk_size: args.chunk_size.max(1),
                parallel: args.parallel.max(1),
                start_index: 0,
            })
            .await,
        );
    }
    if !x_sources.is_empty() {
        results.extend(
            fetch_x_sources(
                args.bird_command.clone(),
                output_dir.clone(),
                x_sources.clone(),
                args.x_parallel.max(1),
                args.x_timeout_seconds,
                results.len(),
            )
            .await,
        );
    }

    results.sort_by_key(|chunk| chunk.index);
    let ok_chunks = results.iter().filter(|chunk| chunk.ok).count();
    let failed_chunks = results.len().saturating_sub(ok_chunks);
    let result = FetchSourcesResult {
        session_id: artifact.id.clone(),
        output_dir: output_dir.display().to_string(),
        source_count: web_urls.len() + x_sources.len(),
        web_source_count: web_urls.len(),
        x_source_count: x_sources.len(),
        chunk_count: results.len(),
        ok_chunks,
        failed_chunks,
        chunks: results,
    };

    let manifest_path = output_dir.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&result)?)
        .with_context(|| format!("write {}", manifest_path.display()))?;
    Ok(result)
}

pub async fn fetch_source(
    config: &Config,
    artifact: &SearchArtifact,
    args: &FetchSourceArgs,
) -> Result<FetchedSourceResult> {
    let (source_index, source) = select_source(artifact, args)?;
    let output_dir = args
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("sources-{}", artifact.id)));
    fs::create_dir_all(&output_dir).with_context(|| format!("create {}", output_dir.display()))?;

    if source.source_type == SourceType::X || is_x_url(&source.url) {
        let output_file = args.output_file.clone().unwrap_or_else(|| {
            output_dir.join(x_output_filename(source_index.unwrap_or(1) - 1, &source))
        });
        ensure_parent_dir(&output_file)?;
        let chunk = fetch_x_source(
            args.bird_command.clone(),
            output_file,
            source_index.unwrap_or(1) - 1,
            source.url.clone(),
            args.x_timeout_seconds,
        )
        .await;
        return Ok(FetchedSourceResult {
            session_id: artifact.id.clone(),
            source_index,
            url: source.url,
            provider: chunk.provider,
            source_type: chunk.source_type,
            output_file: chunk.output_file,
            ok: chunk.ok,
            error: chunk.error,
        });
    }

    let output_file = args.output_file.clone().unwrap_or_else(|| {
        output_dir.join(format!(
            "web-source-{:03}.md",
            source_index.unwrap_or(1).saturating_sub(1)
        ))
    });
    ensure_parent_dir(&output_file)?;
    let markdown = tavily_extract(config, &source.url, args.timeout_seconds).await?;
    fs::write(&output_file, markdown)
        .with_context(|| format!("write {}", output_file.display()))?;
    Ok(FetchedSourceResult {
        session_id: artifact.id.clone(),
        source_index,
        url: source.url,
        provider: "tavily".to_string(),
        source_type: "web".to_string(),
        output_file: output_file.display().to_string(),
        ok: true,
        error: None,
    })
}

fn select_source(
    artifact: &SearchArtifact,
    args: &FetchSourceArgs,
) -> Result<(Option<usize>, Source)> {
    if let Some(index) = args.index {
        if index == 0 {
            anyhow::bail!("--index is one-based and must be greater than 0");
        }
        let source = artifact
            .sources
            .get(index - 1)
            .cloned()
            .with_context(|| format!("source index {index} not found"))?;
        return Ok((Some(index), source));
    }

    if let Some(url) = &args.url {
        if let Some((idx, source)) = artifact
            .sources
            .iter()
            .enumerate()
            .find(|(_, source)| source.url == *url)
        {
            return Ok((Some(idx + 1), source.clone()));
        }
        let source_type = if is_x_url(url) {
            SourceType::X
        } else {
            SourceType::Web
        };
        return Ok((
            None,
            Source {
                url: url.clone(),
                title: None,
                description: None,
                provider: if source_type == SourceType::X {
                    "x".to_string()
                } else {
                    "manual".to_string()
                },
                source_type,
                provenance: vec!["fetch_source_url".to_string()],
                observed_at: Utc::now(),
                handle: None,
                post_id: None,
            },
        ));
    }

    anyhow::bail!("provide --index or --url")
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    Ok(())
}

async fn fetch_web_source_chunks(batch: WebSourceBatch) -> Vec<FetchSourcesChunk> {
    let chunks: Vec<Vec<String>> = batch
        .urls
        .chunks(batch.chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect();
    let chunk_count = chunks.len();
    let mut next = 0usize;
    let mut set = JoinSet::new();
    let mut results = Vec::new();

    while next < chunk_count || !set.is_empty() {
        while next < chunk_count && set.len() < batch.parallel {
            let chunk_urls = chunks[next].clone();
            let client = batch.client.clone();
            let api_key = batch.api_key.clone();
            let api_url = batch.api_url.clone();
            let output_file = batch.output_dir.join(format!("web-{next:03}.json"));
            let index = batch.start_index + next;
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
                    provider: "tavily".to_string(),
                    source_type: "web".to_string(),
                    url_count: 0,
                    urls: Vec::new(),
                    output_file: String::new(),
                    ok: false,
                    error: Some(err.to_string()),
                },
            });
        }
    }

    results.sort_by_key(|chunk| chunk.index);
    results
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
    let manifest_urls = urls.clone();
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
                        provider: "tavily".to_string(),
                        source_type: "web".to_string(),
                        url_count,
                        urls: manifest_urls,
                        output_file: output_file_string,
                        ok: true,
                        error: None,
                    },
                    Err(err) => FetchSourcesChunk {
                        index,
                        provider: "tavily".to_string(),
                        source_type: "web".to_string(),
                        url_count,
                        urls: manifest_urls,
                        output_file: output_file_string,
                        ok: false,
                        error: Some(err.to_string()),
                    },
                }
            } else {
                FetchSourcesChunk {
                    index,
                    provider: "tavily".to_string(),
                    source_type: "web".to_string(),
                    url_count,
                    urls: manifest_urls,
                    output_file: output_file_string,
                    ok: false,
                    error: Some(format!("HTTP {}: {}", status, text)),
                }
            }
        }
        Err(err) => FetchSourcesChunk {
            index,
            provider: "tavily".to_string(),
            source_type: "web".to_string(),
            url_count,
            urls: manifest_urls,
            output_file: output_file_string,
            ok: false,
            error: Some(err.to_string()),
        },
    }
}

async fn fetch_x_sources(
    bird_command: String,
    output_dir: PathBuf,
    sources: Vec<Source>,
    parallel: usize,
    timeout_seconds: u64,
    start_index: usize,
) -> Vec<FetchSourcesChunk> {
    let mut next = 0usize;
    let source_count = sources.len();
    let mut set = JoinSet::new();
    let mut results = Vec::new();

    while next < source_count || !set.is_empty() {
        while next < source_count && set.len() < parallel {
            let source = sources[next].clone();
            let bird_command = bird_command.clone();
            let output_file = output_dir.join(x_output_filename(next, &source));
            let index = start_index + next;
            set.spawn(async move {
                fetch_x_source(
                    bird_command,
                    output_file,
                    index,
                    source.url,
                    timeout_seconds,
                )
                .await
            });
            next += 1;
        }

        if let Some(joined) = set.join_next().await {
            results.push(match joined {
                Ok(chunk) => chunk,
                Err(err) => FetchSourcesChunk {
                    index: usize::MAX,
                    provider: "bird".to_string(),
                    source_type: "x".to_string(),
                    url_count: 0,
                    urls: Vec::new(),
                    output_file: String::new(),
                    ok: false,
                    error: Some(err.to_string()),
                },
            });
        }
    }

    results.sort_by_key(|chunk| chunk.index);
    results
}

async fn fetch_x_source(
    bird_command: String,
    output_file: PathBuf,
    index: usize,
    url: String,
    timeout_seconds: u64,
) -> FetchSourcesChunk {
    let output_file_string = output_file.display().to_string();
    let mut command = Command::new(&bird_command);
    command
        .arg("--timeout")
        .arg((timeout_seconds.saturating_mul(1000)).to_string())
        .arg("--quote-depth")
        .arg("1")
        .arg("read")
        .arg(&url)
        .arg("--json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = match command.spawn() {
        Ok(child) => timeout(
            Duration::from_secs(timeout_seconds + 2),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("bird command timed out"))
        .and_then(|result| result.map_err(anyhow::Error::from)),
        Err(err) => Err(anyhow::Error::from(err)),
    };

    match output {
        Ok(output) if output.status.success() => match fs::write(&output_file, output.stdout) {
            Ok(()) => FetchSourcesChunk {
                index,
                provider: "bird".to_string(),
                source_type: "x".to_string(),
                url_count: 1,
                urls: vec![url],
                output_file: output_file_string,
                ok: true,
                error: None,
            },
            Err(err) => FetchSourcesChunk {
                index,
                provider: "bird".to_string(),
                source_type: "x".to_string(),
                url_count: 1,
                urls: vec![url],
                output_file: output_file_string,
                ok: false,
                error: Some(err.to_string()),
            },
        },
        Ok(output) => FetchSourcesChunk {
            index,
            provider: "bird".to_string(),
            source_type: "x".to_string(),
            url_count: 1,
            urls: vec![url],
            output_file: output_file_string,
            ok: false,
            error: Some(format!(
                "bird exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        },
        Err(err) => FetchSourcesChunk {
            index,
            provider: "bird".to_string(),
            source_type: "x".to_string(),
            url_count: 1,
            urls: vec![url],
            output_file: output_file_string,
            ok: false,
            error: Some(err.to_string()),
        },
    }
}

fn x_output_filename(index: usize, source: &Source) -> String {
    source
        .post_id
        .as_deref()
        .map(|post_id| format!("x-{index:03}-{post_id}.json"))
        .unwrap_or_else(|| format!("x-{index:03}.json"))
}

fn is_x_url(url: &str) -> bool {
    url.contains("://x.com/") || url.contains("://twitter.com/")
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
