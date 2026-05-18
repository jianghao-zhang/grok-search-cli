use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "grok-search-cli")]
#[command(about = "Grok-centered realtime source reconnaissance CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    /// Run a Grok realtime source reconnaissance search and persist a session artifact.
    Search(SearchArgs),
    /// Show sources from a persisted search artifact.
    Sources(SourcesArgs),
    /// Fetch full page content through Tavily Extract, with Firecrawl fallback when configured.
    Fetch(FetchArgs),
    /// Map a site through Tavily Map.
    Map(MapArgs),
    /// List models exposed by the configured Grok-compatible endpoint.
    Models(ModelsArgs),
    /// Show or update local configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Emit a lightweight CLI execution plan for complex searches.
    Plan(PlanArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SearchArgs {
    /// Search query. Multiple words are joined with spaces.
    #[arg(required = true, num_args = 1..)]
    pub query: Vec<String>,

    /// Per-request model override.
    #[arg(long)]
    pub model: Option<String>,

    /// Disable Grok Web Search. Web Search is enabled by default unless this flag is set.
    #[arg(long)]
    pub no_web_search: bool,

    /// Enable Grok X Search.
    #[arg(long)]
    pub x_search: bool,

    /// Optional human-readable platform focus, e.g. "X", "GitHub", "Reddit".
    #[arg(long)]
    pub platform: Option<String>,

    /// Restrict Web Search to these domains. May be repeated.
    #[arg(long)]
    pub allowed_domain: Vec<String>,

    /// Exclude these domains from Web Search. May be repeated.
    #[arg(long)]
    pub excluded_domain: Vec<String>,

    /// Restrict X Search to these handles, without @. May be repeated.
    #[arg(long)]
    pub allowed_x_handle: Vec<String>,

    /// Exclude these X handles, without @. May be repeated.
    #[arg(long)]
    pub excluded_x_handle: Vec<String>,

    /// Start date for X Search or temporal evidence weighting, ISO 8601 date.
    #[arg(long, alias = "since")]
    pub from_date: Option<String>,

    /// End date for X Search or temporal evidence weighting, ISO 8601 date.
    #[arg(long, alias = "until")]
    pub to_date: Option<String>,

    /// Set from-date to today minus this many days when from-date is omitted.
    #[arg(long)]
    pub recency_days: Option<i64>,

    /// Prefer newer evidence when sources conflict.
    #[arg(long)]
    pub prefer_recent: bool,

    /// Conflict display policy for contradictory evidence.
    #[arg(long, default_value = "show-all")]
    pub conflict_policy: ConflictPolicy,

    /// Enable image understanding for tool-backed search.
    #[arg(long)]
    pub enable_image_understanding: bool,

    /// Enable video understanding for X Search.
    #[arg(long)]
    pub enable_video_understanding: bool,

    /// Explicitly add extra Tavily/Firecrawl search-source candidates.
    #[arg(long, default_value_t = 0)]
    pub extra_sources: usize,

    /// Maximum model output tokens.
    #[arg(long)]
    pub max_output_tokens: Option<u32>,

    /// Request timeout in seconds.
    #[arg(long, default_value_t = 90)]
    pub timeout_seconds: u64,

    /// Include raw upstream JSON in the saved artifact.
    #[arg(long)]
    pub include_raw: bool,

    /// Override artifact directory.
    #[arg(long)]
    pub artifact_dir: Option<PathBuf>,

    /// Output format.
    #[arg(long, default_value = "text")]
    pub format: OutputFormat,
}

impl SearchArgs {
    pub fn query_text(&self) -> String {
        self.query.join(" ")
    }
}

#[derive(Args, Debug, Clone)]
pub struct SourcesArgs {
    /// Artifact session id or path to artifact JSON.
    pub session: String,

    /// Override artifact directory when session is an id.
    #[arg(long)]
    pub artifact_dir: Option<PathBuf>,

    /// Output format.
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct FetchArgs {
    /// URL to extract.
    pub url: String,

    /// Request timeout in seconds.
    #[arg(long, default_value_t = 90)]
    pub timeout_seconds: u64,

    /// Output format.
    #[arg(long, default_value = "text")]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct MapArgs {
    /// Root URL to map.
    pub url: String,

    /// Natural language filtering instructions.
    #[arg(long, default_value = "")]
    pub instructions: String,

    /// Maximum traversal depth.
    #[arg(long, default_value_t = 1)]
    pub max_depth: u32,

    /// Maximum links to follow per page.
    #[arg(long, default_value_t = 20)]
    pub max_breadth: u32,

    /// Total link processing limit.
    #[arg(long, default_value_t = 50)]
    pub limit: u32,

    /// Request timeout in seconds.
    #[arg(long, default_value_t = 150)]
    pub timeout_seconds: u64,

    /// Output format.
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct ModelsArgs {
    /// Request timeout in seconds.
    #[arg(long, default_value_t = 10)]
    pub timeout_seconds: u64,

    /// Output format.
    #[arg(long, default_value = "text")]
    pub format: OutputFormat,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Show masked configuration.
    Show(ConfigShowArgs),
    /// Persist the default Grok model in ~/.config/grok-search/config.json.
    SetModel(SetModelArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ConfigShowArgs {
    /// Output format.
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct SetModelArgs {
    pub model: String,
}

#[derive(Args, Debug, Clone)]
pub struct PlanArgs {
    /// User query to plan.
    #[arg(required = true, num_args = 1..)]
    pub query: Vec<String>,

    /// Output format.
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

impl PlanArgs {
    pub fn query_text(&self) -> String {
        self.query.join(" ")
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum ConflictPolicy {
    PreferNewer,
    PreferAuthority,
    ShowAll,
}

impl std::fmt::Display for ConflictPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PreferNewer => write!(f, "prefer-newer"),
            Self::PreferAuthority => write!(f, "prefer-authority"),
            Self::ShowAll => write!(f, "show-all"),
        }
    }
}
