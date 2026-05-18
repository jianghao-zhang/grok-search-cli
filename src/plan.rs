use chrono::{Duration, Local};
use serde_json::{Value, json};

use crate::cli::PlanArgs;
use crate::source::extract_urls;

pub fn build_plan(args: &PlanArgs) -> Value {
    let query = args.query_text();
    let lower = query.to_lowercase();
    let urls = extract_urls(&query);
    let x_related = lower.contains(" x ")
        || lower.contains("twitter")
        || lower.contains("tweet")
        || lower.contains("community")
        || lower.contains("社区");
    let realtime = is_realtime(&query);
    let recent_from = if realtime {
        Some(
            (Local::now().date_naive() - Duration::days(14))
                .format("%Y-%m-%d")
                .to_string(),
        )
    } else {
        None
    };

    let mut commands = Vec::new();
    if !urls.is_empty() {
        for url in &urls {
            commands.push(json!({
                "command": "fetch",
                "args": [url],
                "purpose": "read a known source URL"
            }));
        }
    }

    let mut search_args = vec!["search".to_string()];
    if x_related {
        search_args.push("--x-search".to_string());
    }
    if let Some(from) = &recent_from {
        search_args.push("--from-date".to_string());
        search_args.push(from.clone());
        search_args.push("--prefer-recent".to_string());
    }
    search_args.push(query.clone());
    commands.push(json!({
        "command": "search",
        "args": search_args,
        "purpose": if x_related {
            "X community source reconnaissance"
        } else {
            "Grok realtime source reconnaissance"
        }
    }));

    json!({
        "core_question": query,
        "time_sensitivity": if realtime { "realtime_or_recent" } else { "unknown_or_historical" },
        "recommended_surface": if x_related { "x_search_surface" } else { "web_search_surface" },
        "commands": commands,
        "notes": [
            "This is a CLI execution plan, not a mandatory pre-search phase.",
            "External verification remains the agent workflow's responsibility."
        ]
    })
}

fn is_realtime(query: &str) -> bool {
    let lower = query.to_lowercase();
    [
        "latest",
        "recent",
        "today",
        "now",
        "current",
        "real-time",
        "realtime",
        "最新",
        "最近",
        "今天",
        "现在",
        "当前",
        "实时",
    ]
    .iter()
    .any(|needle| lower.contains(needle) || query.contains(needle))
}
