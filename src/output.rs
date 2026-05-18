use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::{artifact::SearchArtifact, cli::OutputFormat, config::Config, web_tools::FetchResult};

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

pub fn print_sources(artifact: &SearchArtifact, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(&artifact.sources),
        OutputFormat::Text => {
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
            println!("firecrawl_api_url: {}", masked.firecrawl_api_url);
            println!("firecrawl_api_key: {}", masked.firecrawl_api_key);
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
