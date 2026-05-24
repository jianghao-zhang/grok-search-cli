use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{artifact::SearchArtifact, cli::SearchArgs, config::Config, grok};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobRecord {
    pub id: String,
    pub kind: JobKind,
    pub status: JobStatus,
    pub query: String,
    pub search_args: SearchArgs,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub pid: Option<u32>,
    pub timeout_seconds: u64,
    pub session_id: Option<String>,
    pub artifact_path: Option<PathBuf>,
    pub log_file: PathBuf,
    pub error: Option<String>,
    pub cancel_requested: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Search,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

pub fn submit_search(mut args: SearchArgs) -> Result<JobRecord> {
    args.background = false;

    let id = fresh_job_id();
    let now = Utc::now();
    let log_file = jobs_dir().join(format!("{id}.log"));
    let job = JobRecord {
        id,
        kind: JobKind::Search,
        status: JobStatus::Queued,
        query: args.query_text(),
        timeout_seconds: args.timeout_seconds,
        search_args: args,
        created_at: now,
        started_at: None,
        updated_at: now,
        completed_at: None,
        pid: None,
        session_id: None,
        artifact_path: None,
        log_file,
        error: None,
        cancel_requested: false,
    };

    save_job(&job)?;
    let pid = spawn_worker(&job)?;
    let mut current = read_job(&job.id).unwrap_or(job);
    if !current.status.is_terminal() {
        current.pid = Some(pid);
        current.updated_at = Utc::now();
        save_job(&current)?;
    }
    Ok(current)
}

pub async fn run_search_job(config: &Config, job_id: &str) -> Result<()> {
    let mut job = read_job(job_id)?;
    if job.status.is_terminal() {
        bail!("job {} is already {}", job.id, job.status.as_str());
    }
    if job.cancel_requested {
        mark_cancelled(job_id)?;
        return Ok(());
    }

    job.status = JobStatus::Running;
    job.pid = Some(std::process::id());
    job.started_at = Some(Utc::now());
    job.updated_at = job.started_at.unwrap();
    save_job(&job)?;

    let heartbeat_job_id = job.id.clone();
    let heartbeat = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            match touch_running(&heartbeat_job_id) {
                Ok(HeartbeatAction::Continue) => {}
                Ok(HeartbeatAction::Cancelled) => std::process::exit(130),
                Err(_) => {}
            }
        }
    });

    let result = grok::search(config, job.search_args.clone()).await;
    heartbeat.abort();

    match result {
        Ok(artifact) => {
            mark_succeeded(job_id, &artifact)?;
            println!("job_id: {}", job_id);
            println!("status: succeeded");
            println!("session_id: {}", artifact.id);
            if let Some(path) = artifact.artifact_path {
                println!("artifact: {}", path.display());
            }
            Ok(())
        }
        Err(err) => {
            mark_failed(job_id, &err.to_string())?;
            Err(err)
        }
    }
}

pub fn status_job(job_id: &str) -> Result<JobRecord> {
    let job = read_job(job_id)?;
    reconcile_dead_worker(job)
}

pub fn list_jobs(limit: usize) -> Result<Vec<JobRecord>> {
    let dir = jobs_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut jobs = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let job: JobRecord =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
        jobs.push(reconcile_dead_worker(job)?);
    }

    jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    jobs.truncate(limit);
    Ok(jobs)
}

pub fn cancel_job(job_id: &str) -> Result<JobRecord> {
    let mut job = read_job(job_id)?;
    if job.status.is_terminal() {
        return Ok(job);
    }

    job.cancel_requested = true;
    job.status = JobStatus::Cancelled;
    job.completed_at = Some(Utc::now());
    job.updated_at = job.completed_at.unwrap();
    if let Some(pid) = job.pid {
        terminate_pid(pid);
    }
    save_job(&job)?;
    Ok(job)
}

pub fn completed_artifact(job_id: &str) -> Result<SearchArtifact> {
    let job = status_job(job_id)?;
    match job.status {
        JobStatus::Succeeded => {
            let path = job
                .artifact_path
                .as_ref()
                .context("job succeeded without an artifact path")?;
            crate::artifact::load(&path.display().to_string(), None)
        }
        JobStatus::Failed => bail!(
            "job {} failed: {}",
            job.id,
            job.error.unwrap_or_else(|| "unknown error".to_string())
        ),
        JobStatus::Cancelled => bail!("job {} was cancelled", job.id),
        JobStatus::Queued | JobStatus::Running => {
            bail!("job {} is still {}", job.id, job.status.as_str())
        }
    }
}

pub async fn wait_job(job_id: &str, poll_seconds: u64, timeout_seconds: u64) -> Result<JobRecord> {
    let started_at = Utc::now();
    let poll_seconds = poll_seconds.max(1);
    loop {
        let job = status_job(job_id)?;
        if job.status.is_terminal() {
            return Ok(job);
        }

        if timeout_seconds > 0
            && Utc::now().signed_duration_since(started_at).num_seconds() >= timeout_seconds as i64
        {
            bail!(
                "job {} is still {} after {} seconds",
                job.id,
                job.status.as_str(),
                timeout_seconds
            );
        }

        tokio::time::sleep(Duration::from_secs(poll_seconds)).await;
    }
}

fn touch_running(job_id: &str) -> Result<HeartbeatAction> {
    let mut job = read_job(job_id)?;
    if job.cancel_requested || job.status == JobStatus::Cancelled {
        job.status = JobStatus::Cancelled;
        job.completed_at.get_or_insert_with(Utc::now);
        job.updated_at = Utc::now();
        save_job(&job)?;
        return Ok(HeartbeatAction::Cancelled);
    }
    if job.status == JobStatus::Running {
        job.updated_at = Utc::now();
        save_job(&job)?;
    }
    Ok(HeartbeatAction::Continue)
}

enum HeartbeatAction {
    Continue,
    Cancelled,
}

fn mark_succeeded(job_id: &str, artifact: &SearchArtifact) -> Result<()> {
    let mut job = read_job(job_id)?;
    let now = Utc::now();
    job.status = JobStatus::Succeeded;
    job.updated_at = now;
    job.completed_at = Some(now);
    job.session_id = Some(artifact.id.clone());
    job.artifact_path = artifact.artifact_path.clone();
    job.error = None;
    save_job(&job)
}

fn mark_failed(job_id: &str, error: &str) -> Result<()> {
    let mut job = read_job(job_id)?;
    let now = Utc::now();
    job.status = JobStatus::Failed;
    job.updated_at = now;
    job.completed_at = Some(now);
    job.error = Some(error.to_string());
    save_job(&job)
}

fn mark_cancelled(job_id: &str) -> Result<()> {
    let mut job = read_job(job_id)?;
    let now = Utc::now();
    job.status = JobStatus::Cancelled;
    job.cancel_requested = true;
    job.updated_at = now;
    job.completed_at = Some(now);
    save_job(&job)
}

fn reconcile_dead_worker(mut job: JobRecord) -> Result<JobRecord> {
    if job.status != JobStatus::Running {
        return Ok(job);
    }
    let Some(pid) = job.pid else {
        return Ok(job);
    };
    if pid_is_alive(pid) {
        return Ok(job);
    }

    let now = Utc::now();
    job.status = JobStatus::Failed;
    job.updated_at = now;
    job.completed_at = Some(now);
    job.error
        .get_or_insert_with(|| "worker process exited before writing a final result".to_string());
    save_job(&job)?;
    Ok(job)
}

fn spawn_worker(job: &JobRecord) -> Result<u32> {
    if let Some(parent) = job.log_file.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&job.log_file)
        .with_context(|| format!("open {}", job.log_file.display()))?;
    let stderr = stdout
        .try_clone()
        .with_context(|| format!("clone {}", job.log_file.display()))?;

    let exe = std::env::current_exe().context("resolve current executable")?;
    let mut command = Command::new(exe);
    command
        .arg("job")
        .arg("run")
        .arg(&job.id)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    detach_child(&mut command);

    let child = command.spawn().context("spawn background search worker")?;
    Ok(child.id())
}

#[cfg(unix)]
fn detach_child(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    unsafe extern "C" {
        fn setsid() -> i32;
    }

    // Safety: the child only calls setsid before exec to detach from the
    // caller's terminal/session; no Rust state is touched in the child path.
    unsafe {
        command.pre_exec(|| {
            if setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn detach_child(_: &mut Command) {}

fn read_job(job_id: &str) -> Result<JobRecord> {
    let path = job_path(job_id);
    if !path.exists() {
        bail!("job not found: {}", job_id);
    }
    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn save_job(job: &JobRecord) -> Result<()> {
    let path = job_path(&job.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(job)?)
        .with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("rename {} to {}", tmp.display(), path.display()))?;
    Ok(())
}

fn job_path(job_id: &str) -> PathBuf {
    jobs_dir().join(format!("{job_id}.json"))
}

fn jobs_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("grok-search")
        .join("jobs")
}

fn fresh_job_id() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_string()
}

fn pid_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn terminate_pid(pid: u32) {
    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}
