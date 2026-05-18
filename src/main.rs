mod artifact;
mod cli;
mod config;
mod doctor;
mod grok;
mod output;
mod plan;
mod source;
mod web_tools;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, ConfigCommand, SourceCommand, WebCommand};
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Commands::Search(args) => {
            let artifact = grok::search(&config, args).await?;
            output::print_search_artifact(&artifact, artifact.format)?;
        }
        Commands::Source { command } => match command {
            SourceCommand::Index(args) => {
                let args = args.into_sources_args();
                let artifact = artifact::load(&args.session, args.artifact_dir.as_deref())?;
                output::print_sources(&artifact, &args)?;
            }
            SourceCommand::Links(args) => {
                let args = args.into_sources_args();
                let artifact = artifact::load(&args.session, args.artifact_dir.as_deref())?;
                output::print_sources(&artifact, &args)?;
            }
            SourceCommand::Fetch(args) => {
                let artifact = artifact::load(&args.session, args.artifact_dir.as_deref())?;
                let result = web_tools::fetch_source(&config, &artifact, &args).await?;
                output::print_fetched_source(&result, args.format)?;
            }
            SourceCommand::FetchAll(args) => {
                let artifact = artifact::load(&args.session, args.artifact_dir.as_deref())?;
                let result = web_tools::fetch_sources(&config, &artifact, &args).await?;
                output::print_fetch_sources(&result, args.format)?;
            }
        },
        Commands::Web { command } => match command {
            WebCommand::Fetch(args) => {
                let fetched = web_tools::fetch(&config, &args).await?;
                output::print_fetch(&fetched, args.format)?;
            }
            WebCommand::Map(args) => {
                let mapped = web_tools::map(&config, &args).await?;
                output::print_json_or_text(&mapped, args.format)?;
            }
        },
        Commands::Sources(args) => {
            let artifact = artifact::load(&args.session, args.artifact_dir.as_deref())?;
            output::print_sources(&artifact, &args)?;
        }
        Commands::FetchSources(args) => {
            let artifact = artifact::load(&args.session, args.artifact_dir.as_deref())?;
            let result = web_tools::fetch_sources(&config, &artifact, &args).await?;
            output::print_fetch_sources(&result, args.format)?;
        }
        Commands::FetchSource(args) => {
            let artifact = artifact::load(&args.session, args.artifact_dir.as_deref())?;
            let result = web_tools::fetch_source(&config, &artifact, &args).await?;
            output::print_fetched_source(&result, args.format)?;
        }
        Commands::Fetch(args) => {
            let fetched = web_tools::fetch(&config, &args).await?;
            output::print_fetch(&fetched, args.format)?;
        }
        Commands::Map(args) => {
            let mapped = web_tools::map(&config, &args).await?;
            output::print_json_or_text(&mapped, args.format)?;
        }
        Commands::Models(args) => {
            let models = grok::models(&config, args.timeout_seconds).await?;
            output::print_models(&models, args.format)?;
        }
        Commands::Doctor(args) => {
            let report = doctor::run(&config, &args).await?;
            output::print_json_or_text(&report, args.format)?;
        }
        Commands::Config { command } => match command {
            ConfigCommand::Show(args) => output::print_config(&config, args.format)?,
            ConfigCommand::SetModel(args) => {
                config::set_model(&args.model)?;
                println!("model set to {}", args.model);
            }
            ConfigCommand::SetTavilyKey(args) => {
                config::set_tavily_key(&args.key)?;
                println!("tavily api key set");
            }
        },
        Commands::Plan(args) => {
            let plan = plan::build_plan(&args);
            output::print_json_or_text(&plan, args.format)?;
        }
    }

    Ok(())
}
