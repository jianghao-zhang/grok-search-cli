use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::{
    artifact::SearchArtifact,
    cli::{OutputFormat, SourcesArgs, SourcesFormat},
    config::Config,
    jobs::JobRecord,
    source::{Source, SourceType},
    web_tools::{FetchResult, FetchSourcesResult, FetchedSourceResult},
};

pub fn print_search_artifact(artifact: &SearchArtifact, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(artifact),
        OutputFormat::Text => {
            println!("{}", artifact.answer.trim());
            println!();
            println!("session_id: {}", artifact.id);
            if let Some(path) = &artifact.artifact_path {
                println!("artifact: {}", path.display());
            }
            println!("sources_count: {}", artifact.sources.len());
            println!("tool_calls: {}", artifact.tool_calls.len());
            if !artifact.warnings.is_empty() {
                println!("warnings: {}", artifact.warnings.join(", "));
            }
            Ok(())
        }
    }
}

pub fn print_sources(artifact: &SearchArtifact, args: &SourcesArgs) -> Result<()> {
    let rendered = render_sources(artifact, args)?;
    if args.write || args.output_file.is_some() {
        let path = args
            .output_file
            .clone()
            .unwrap_or_else(|| default_sources_output_file(artifact, args.format));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::write(&path, rendered).with_context(|| format!("write {}", path.display()))?;
        println!("output_file: {}", path.display());
        println!("source_count: {}", artifact.sources.len());
        return Ok(());
    }
    print!("{rendered}");
    Ok(())
}

pub fn print_fetch(result: &FetchResult, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(result),
        OutputFormat::Text => {
            println!("{}", result.markdown);
            Ok(())
        }
    }
}

pub fn print_fetch_sources(result: &FetchSourcesResult, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(result),
        OutputFormat::Text => {
            println!("session_id: {}", result.session_id);
            println!("output_dir: {}", result.output_dir);
            println!("source_count: {}", result.source_count);
            println!("web_source_count: {}", result.web_source_count);
            println!("x_source_count: {}", result.x_source_count);
            println!("chunk_count: {}", result.chunk_count);
            println!("ok_chunks: {}", result.ok_chunks);
            println!("failed_chunks: {}", result.failed_chunks);
            for chunk in &result.chunks {
                println!(
                    "chunk {} [{}:{}]: {} urls -> {} ({}; {} ms; {} bytes)",
                    chunk.index,
                    chunk.provider,
                    chunk.source_type,
                    chunk.url_count,
                    chunk.output_file,
                    if chunk.ok { "ok" } else { "failed" },
                    chunk.latency_ms,
                    chunk.bytes
                );
                if let Some(error) = &chunk.error {
                    println!("  error: {error}");
                }
            }
            Ok(())
        }
    }
}

pub fn print_fetched_source(result: &FetchedSourceResult, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(result),
        OutputFormat::Text => {
            println!("session_id: {}", result.session_id);
            if let Some(index) = result.source_index {
                println!("source_index: {index}");
            }
            println!("url: {}", result.url);
            println!("provider: {}", result.provider);
            println!("source_type: {}", result.source_type);
            println!("output_file: {}", result.output_file);
            println!("latency_ms: {}", result.latency_ms);
            println!("bytes: {}", result.bytes);
            println!("ok: {}", result.ok);
            if let Some(error) = &result.error {
                println!("error: {error}");
            }
            Ok(())
        }
    }
}

pub fn print_models(models: &[String], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(models),
        OutputFormat::Text => {
            for model in models {
                println!("{model}");
            }
            Ok(())
        }
    }
}

pub fn print_job_submitted(job: &JobRecord, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(job),
        OutputFormat::Text => {
            println!("job_id: {}", job.id);
            println!("status: {}", job.status.as_str());
            if let Some(pid) = job.pid {
                println!("pid: {pid}");
            }
            println!("query: {}", job.query);
            println!("created_at: {}", job.created_at);
            println!("log_file: {}", job.log_file.display());
            println!("status_command: grok-search-cli job status {}", job.id);
            println!("result_command: grok-search-cli job result {}", job.id);
            Ok(())
        }
    }
}

pub fn print_job(job: &JobRecord, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(job),
        OutputFormat::Text => {
            println!("job_id: {}", job.id);
            println!("status: {}", job.status.as_str());
            println!("query: {}", job.query);
            if let Some(pid) = job.pid {
                println!("pid: {pid}");
            }
            println!("created_at: {}", job.created_at);
            if let Some(started_at) = job.started_at {
                println!("started_at: {started_at}");
            }
            println!("updated_at: {}", job.updated_at);
            if let Some(completed_at) = job.completed_at {
                println!("completed_at: {completed_at}");
            }
            println!("elapsed_seconds: {}", job_elapsed_seconds(job));
            if let Some(session_id) = &job.session_id {
                println!("session_id: {session_id}");
            }
            if let Some(path) = &job.artifact_path {
                println!("artifact: {}", path.display());
            }
            println!("log_file: {}", job.log_file.display());
            if let Some(error) = &job.error {
                println!("error: {error}");
            }
            Ok(())
        }
    }
}

pub fn print_jobs(jobs: &[JobRecord], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(jobs),
        OutputFormat::Text => {
            if jobs.is_empty() {
                println!("no jobs");
                return Ok(());
            }
            for job in jobs {
                println!(
                    "{}  {:<9}  {:>5}s  {}",
                    job.id,
                    job.status.as_str(),
                    job_elapsed_seconds(job),
                    truncate(&job.query, 96)
                );
            }
            Ok(())
        }
    }
}

pub fn print_config(config: &Config, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(&config.masked()),
        OutputFormat::Text => {
            let masked = config.masked();
            println!("config_file: {}", masked.config_file);
            println!("grok_api_url: {}", masked.grok_api_url);
            println!("grok_api_key: {}", masked.grok_api_key);
            println!("grok_model: {}", masked.grok_model);
            println!("tavily_api_url: {}", masked.tavily_api_url);
            println!("tavily_api_key: {}", masked.tavily_api_key);
            Ok(())
        }
    }
}

pub fn print_json_or_text(value: &Value, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(value),
        OutputFormat::Text => {
            if let Some(text) = value.as_str() {
                println!("{text}");
            } else {
                println!("{}", serde_json::to_string_pretty(value)?);
            }
            Ok(())
        }
    }
}

fn print_json<T: Serialize + ?Sized>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn job_elapsed_seconds(job: &JobRecord) -> i64 {
    let start = job.started_at.unwrap_or(job.created_at);
    let end = job.completed_at.unwrap_or_else(chrono::Utc::now);
    end.signed_duration_since(start).num_seconds().max(0)
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn render_sources(artifact: &SearchArtifact, args: &SourcesArgs) -> Result<String> {
    match args.format {
        SourcesFormat::Json => Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&artifact.sources)?
        )),
        SourcesFormat::Text => Ok(render_text_sources(artifact)),
        SourcesFormat::Markdown => Ok(render_markdown_sources(artifact, args)),
        SourcesFormat::Urls => Ok(render_source_urls(artifact, args)),
        SourcesFormat::FetchCommand | SourcesFormat::TavilyCommand => {
            Ok(format!("{}\n", source_fetch_command(artifact, args)?))
        }
    }
}

fn render_text_sources(artifact: &SearchArtifact) -> String {
    let mut out = String::new();
    for (idx, source) in artifact.sources.iter().enumerate() {
        let title = source.title.as_deref().unwrap_or(&source.url);
        out.push_str(&format!(
            "{}. {} ({:?})\n",
            idx + 1,
            title,
            source.source_type
        ));
        out.push_str(&format!("   {}\n", source.url));
        if let Some(handle) = &source.handle {
            out.push_str(&format!("   handle: @{handle}\n"));
        }
        if let Some(post_id) = &source.post_id {
            out.push_str(&format!("   post_id: {post_id}\n"));
        }
    }
    out
}

fn render_markdown_sources(artifact: &SearchArtifact, args: &SourcesArgs) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Source Index: {}\n\n", artifact.id));
    out.push_str(&format!("- query: {}\n", artifact.query));
    out.push_str(&format!("- source_count: {}\n", artifact.sources.len()));
    out.push('\n');
    for (idx, source) in artifact.sources.iter().enumerate() {
        let title = source.title.as_deref().unwrap_or(&source.url);
        out.push_str(&format!(
            "## {}. {:?}: {}\n\n",
            idx + 1,
            source.source_type,
            title
        ));
        out.push_str(&format!("- url: {}\n", source.url));
        out.push_str(&format!(
            "- fetch: `{}`\n",
            fetch_source_command(args, idx + 1)
        ));
        if let Some(handle) = &source.handle {
            out.push_str(&format!("- handle: @{}\n", handle));
        }
        if let Some(post_id) = &source.post_id {
            out.push_str(&format!("- post_id: {}\n", post_id));
        }
        if !source.provenance.is_empty() {
            out.push_str(&format!("- provenance: {}\n", source.provenance.join(", ")));
        }
        out.push('\n');
    }
    out
}

fn render_source_urls(artifact: &SearchArtifact, args: &SourcesArgs) -> String {
    let mut out = String::new();
    for source in artifact
        .sources
        .iter()
        .filter(|source| source_matches_url_filter(source, args))
    {
        out.push_str(&source.url);
        out.push('\n');
    }
    out
}

fn source_matches_url_filter(source: &Source, args: &SourcesArgs) -> bool {
    if args.no_x && source.source_type == SourceType::X {
        return false;
    }
    if args.no_web && source.source_type != SourceType::X {
        return false;
    }
    args.include_x || source.source_type != SourceType::X
}

fn default_sources_output_file(artifact: &SearchArtifact, format: SourcesFormat) -> PathBuf {
    let file_name = match format {
        SourcesFormat::Json => "sources.json",
        SourcesFormat::Text => "sources.txt",
        SourcesFormat::Markdown => "source-index.md",
        SourcesFormat::Urls => "links.txt",
        SourcesFormat::FetchCommand | SourcesFormat::TavilyCommand => "fetch-sources.sh",
    };
    PathBuf::from(format!("sources-{}", artifact.id)).join(file_name)
}

fn source_fetch_command(artifact: &SearchArtifact, args: &SourcesArgs) -> Result<String> {
    if artifact.sources.is_empty() {
        return Ok("printf '%s\\n' 'No sources found in artifact.' >&2; exit 1".to_string());
    }

    let output_dir = args
        .output_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| format!("sources-{}", artifact.id));
    let mut command = format!(
        "grok-search-cli source fetch-all {}{} --parallel {} --chunk-size {} --x-parallel {} --output-dir {}",
        sh_quote(&args.session),
        args.artifact_dir
            .as_ref()
            .map(|p| format!(" --artifact-dir {}", sh_quote(&p.display().to_string())))
            .unwrap_or_default(),
        args.parallel.max(1),
        args.chunk_size.max(1),
        args.x_parallel.max(1),
        sh_quote(&output_dir)
    );
    if args.x_timeout_seconds != 20 {
        command.push_str(&format!(" --x-timeout-seconds {}", args.x_timeout_seconds));
    }
    if args.bird_command != "bird" {
        command.push_str(&format!(" --bird-command {}", sh_quote(&args.bird_command)));
    }
    if args.no_web {
        command.push_str(" --no-web");
    }
    if args.no_x {
        command.push_str(" --no-x");
    }
    Ok(command)
}

fn fetch_source_command(args: &SourcesArgs, index: usize) -> String {
    let mut command = format!(
        "grok-search-cli source fetch {}{} --index {}",
        sh_quote(&args.session),
        args.artifact_dir
            .as_ref()
            .map(|p| format!(" --artifact-dir {}", sh_quote(&p.display().to_string())))
            .unwrap_or_default(),
        index
    );
    if let Some(output_dir) = &args.output_dir {
        command.push_str(&format!(
            " --output-dir {}",
            sh_quote(&output_dir.display().to_string())
        ));
    }
    if args.x_timeout_seconds != 20 {
        command.push_str(&format!(" --x-timeout-seconds {}", args.x_timeout_seconds));
    }
    if args.bird_command != "bird" {
        command.push_str(&format!(" --bird-command {}", sh_quote(&args.bird_command)));
    }
    command
}

fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::sh_quote;

    #[test]
    fn shell_quotes_single_quotes() {
        assert_eq!(sh_quote("a'b"), "'a'\"'\"'b'");
    }
}
