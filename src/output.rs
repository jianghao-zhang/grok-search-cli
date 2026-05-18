use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::{
    artifact::SearchArtifact,
    cli::{OutputFormat, SourcesArgs, SourcesFormat},
    config::Config,
    source::{Source, SourceType},
    web_tools::{FetchResult, FetchSourcesResult},
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
    match args.format {
        SourcesFormat::Json => print_json(&artifact.sources),
        SourcesFormat::Text => {
            for (idx, source) in artifact.sources.iter().enumerate() {
                let title = source.title.as_deref().unwrap_or(&source.url);
                println!("{}. {} ({:?})", idx + 1, title, source.source_type);
                println!("   {}", source.url);
                if let Some(handle) = &source.handle {
                    println!("   handle: @{handle}");
                }
                if let Some(post_id) = &source.post_id {
                    println!("   post_id: {post_id}");
                }
            }
            Ok(())
        }
        SourcesFormat::Urls => {
            for source in tavily_candidate_sources(artifact, args.include_x) {
                println!("{}", source.url);
            }
            Ok(())
        }
        SourcesFormat::TavilyCommand => {
            println!("{}", tavily_extract_command(artifact, args)?);
            Ok(())
        }
    }
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
            println!("chunk_count: {}", result.chunk_count);
            println!("ok_chunks: {}", result.ok_chunks);
            println!("failed_chunks: {}", result.failed_chunks);
            for chunk in &result.chunks {
                println!(
                    "chunk {}: {} urls -> {} ({})",
                    chunk.index,
                    chunk.url_count,
                    chunk.output_file,
                    if chunk.ok { "ok" } else { "failed" }
                );
                if let Some(error) = &chunk.error {
                    println!("  error: {error}");
                }
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

fn tavily_candidate_sources(artifact: &SearchArtifact, include_x: bool) -> Vec<&Source> {
    artifact
        .sources
        .iter()
        .filter(|source| include_x || source.source_type != SourceType::X)
        .collect()
}

fn tavily_extract_command(artifact: &SearchArtifact, args: &SourcesArgs) -> Result<String> {
    let sources = tavily_candidate_sources(artifact, args.include_x);
    if sources.is_empty() {
        return Ok("printf '%s\\n' 'No non-X sources found for Tavily Extract. Use --include-x to include X URLs.' >&2; exit 1".to_string());
    }

    let output_dir = args
        .output_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| format!("tavily-{}", artifact.id));
    Ok(format!(
        "grok-search-cli fetch-sources {}{} --parallel {} --chunk-size {} --output-dir {}{}",
        sh_quote(&args.session),
        args.artifact_dir
            .as_ref()
            .map(|p| format!(" --artifact-dir {}", sh_quote(&p.display().to_string())))
            .unwrap_or_default(),
        args.parallel.max(1),
        args.chunk_size.max(1),
        sh_quote(&output_dir),
        if args.include_x { " --include-x" } else { "" }
    ))
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
