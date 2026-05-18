use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    cli::OutputFormat,
    source::{Source, ToolCall},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchArtifact {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub query: String,
    pub model_requested: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_effective: Option<String>,
    pub tool_summary: ToolSummary,
    pub answer: String,
    pub sources: Vec<Source>,
    pub tool_calls: Vec<ToolCall>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_response: Option<Value>,
    #[serde(skip)]
    pub artifact_path: Option<PathBuf>,
    #[serde(skip)]
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    pub web_search: bool,
    pub x_search: bool,
    pub allowed_domains: Vec<String>,
    pub excluded_domains: Vec<String>,
    pub allowed_x_handles: Vec<String>,
    pub excluded_x_handles: Vec<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub prefer_recent: bool,
    pub conflict_policy: String,
}

impl SearchArtifact {
    pub fn fresh_id() -> String {
        Uuid::new_v4().simple().to_string()[..12].to_string()
    }
}

pub fn save(mut artifact: SearchArtifact, override_dir: Option<&Path>) -> Result<SearchArtifact> {
    let dir = artifact_dir(override_dir);
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join(format!("{}.json", artifact.id));
    fs::write(&path, serde_json::to_vec_pretty(&artifact)?)
        .with_context(|| format!("write {}", path.display()))?;
    artifact.artifact_path = Some(path);
    Ok(artifact)
}

pub fn load(session: &str, override_dir: Option<&Path>) -> Result<SearchArtifact> {
    let path = resolve_artifact_path(session, override_dir);
    if !path.exists() {
        bail!("artifact not found: {}", path.display());
    }
    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let mut artifact: SearchArtifact =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    artifact.artifact_path = Some(path);
    artifact.format = OutputFormat::Json;
    Ok(artifact)
}

pub fn artifact_dir(override_dir: Option<&Path>) -> PathBuf {
    override_dir.map(Path::to_path_buf).unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("grok-search")
            .join("sessions")
    })
}

fn resolve_artifact_path(session: &str, override_dir: Option<&Path>) -> PathBuf {
    let path = PathBuf::from(session);
    if path.exists() || path.extension().is_some() || session.contains('/') {
        path
    } else {
        artifact_dir(override_dir).join(format!("{session}.json"))
    }
}
