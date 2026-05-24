mod artifact;
mod cli;
mod config;
mod doctor;
mod grok;
mod jobs;
mod output;
mod plan;
mod source;
mod web_tools;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, ConfigCommand, JobCommand, OfficialEndpoint, SourceCommand, WebCommand};
use config::{Config, OFFICIAL_XAI_API_URL, OFFICIAL_XAI_EU_API_URL};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Commands::Search(args) => {
            if args.background {
                let format = args.format;
                let job = jobs::submit_search(args)?;
                output::print_job_submitted(&job, format)?;
            } else {
                let artifact = grok::search(&config, args).await?;
                output::print_search_artifact(&artifact, artifact.format)?;
            }
        }
        Commands::Job { command } => match command {
            JobCommand::Status(args) => {
                let job = jobs::status_job(&args.job_id)?;
                output::print_job(&job, args.format)?;
            }
            JobCommand::Result(args) => {
                let artifact = jobs::completed_artifact(&args.job_id)?;
                output::print_search_artifact(&artifact, args.format)?;
            }
            JobCommand::Wait(args) => {
                let job =
                    jobs::wait_job(&args.job_id, args.poll_seconds, args.timeout_seconds).await?;
                match job.status {
                    jobs::JobStatus::Succeeded => {
                        let artifact = jobs::completed_artifact(&args.job_id)?;
                        output::print_search_artifact(&artifact, args.format)?;
                    }
                    jobs::JobStatus::Failed | jobs::JobStatus::Cancelled => {
                        output::print_job(&job, args.format)?;
                        anyhow::bail!("job {} {}", job.id, job.status.as_str());
                    }
                    jobs::JobStatus::Queued | jobs::JobStatus::Running => unreachable!(),
                }
            }
            JobCommand::List(args) => {
                let jobs = jobs::list_jobs(args.limit)?;
                output::print_jobs(&jobs, args.format)?;
            }
            JobCommand::Cancel(args) => {
                let job = jobs::cancel_job(&args.job_id)?;
                output::print_job(&job, args.format)?;
            }
            JobCommand::Run(args) => {
                jobs::run_search_job(&config, &args.job_id).await?;
            }
        },
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
            ConfigCommand::UseOfficial(args) => {
                let api_url = args.api_url.unwrap_or_else(|| match args.endpoint {
                    OfficialEndpoint::Global => OFFICIAL_XAI_API_URL.to_string(),
                    OfficialEndpoint::EuWest1 => OFFICIAL_XAI_EU_API_URL.to_string(),
                });
                config::use_official(args.api_key.as_deref(), &api_url, &args.model)?;
                println!("official xAI API configured: {api_url}");
                println!("model set to {}", args.model);
            }
            ConfigCommand::UseProxy(args) => {
                config::use_proxy(args.api_key.as_deref(), &args.api_url, &args.model)?;
                println!("Grok2API proxy configured: {}", args.api_url);
                println!("model set to {}", args.model);
            }
            ConfigCommand::SetApiUrl(args) => {
                config::set_api_url(&args.api_url)?;
                println!("api url set to {}", args.api_url);
            }
            ConfigCommand::SetApiKey(args) => {
                config::set_api_key(&args.key)?;
                println!("api key set");
            }
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
