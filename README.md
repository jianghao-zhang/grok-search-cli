# grok-search-cli

Rust CLI for Grok-centered realtime source reconnaissance.

`grok-search-cli` is a fast CLI surface for using Grok where Grok is strongest: realtime X/community discovery, fresh source gathering, and high-recall lead finding in domains where yesterday's answer may already be stale.

The main artifact is a disk-backed Source Pack: answer text, sources, X URLs, tool calls, warnings, timestamps, and citation provenance saved as JSON for later agent verification.

## Why This Exists

Grok is unusually useful for realtime and X-adjacent discovery. This CLI turns that strength into a repeatable search workflow with durable source artifacts.

Use it when recency and community evidence matter:

- X launch-day reactions, incidents, rumors, and public threads
- AI research/model/tooling changes where newer evidence should carry more weight
- obscure community sources that broader web search often misses
- Grok-native source gathering before another agent verifies original pages or posts

## Install

Install Rust/Cargo first if `cargo` is not already available:

- Rust/Cargo: https://doc.rust-lang.org/stable/cargo/getting-started/installation.html

```bash
cargo install --git https://github.com/jianghao-zhang/grok-search-cli.git
```

For local development:

```bash
cargo build --release
```

## Optional Tools

These tools are optional. The core `search`, `source index`, and `source links` workflow works without Tavily or bird.

| Tool | Used for | Setup |
| --- | --- | --- |
| Tavily | Fetching web page bodies through `web fetch`, `web map`, web `source fetch`, and web `source fetch-all` | Get a key from [app.tavily.com](https://app.tavily.com), then run `grok-search-cli config set-tavily-key tvly-...` or set `TAVILY_API_KEY`. Tavily's quickstart is at [docs.tavily.com](https://docs.tavily.com/documentation/quickstart). |
| bird CLI | Fetching X/Twitter source bodies through `source fetch` and `source fetch-all` | Install from [bird.fast](https://bird.fast) with Node 20+: `npm install -g @steipete/bird`, then run `bird check`. bird uses your existing browser session and supports stable `--json` output. |

If Tavily is missing, keep using `search`, `source index`, and `source links`; skip web body fetching or use `--no-web` for batch fetches. If bird is missing or not authenticated, X links still appear in the source index and link list; skip X body fetching or use `--no-x`.

## Configuration

The CLI speaks the xAI/OpenAI-compatible Responses API. Use either the official xAI API or a Grok2API/OpenAI-compatible proxy.

Official references: [xAI Responses API](https://docs.x.ai/docs/guides/chat), [X Search](https://docs.x.ai/developers/tools/x-search), and [model aliases](https://docs.x.ai/developers/models).

Official xAI API:

```bash
export XAI_API_KEY="xai-..."
grok-search-cli config use-official
grok-search-cli doctor
```

This writes:

```json
{
  "provider": "xai-official",
  "model": "grok-4.3",
  "api_url": "https://api.x.ai/v1"
}
```

For the EU endpoint:

```bash
grok-search-cli config use-official --endpoint eu-west-1
```

Grok2API or another OpenAI-compatible proxy:

```bash
grok-search-cli config use-proxy \
  --api-url "http://127.0.0.1:8000/v1" \
  --model "grok-4.20-multi-agent-console"
```

The local config file lives at `~/.config/grok-search/config.json`:

```json
{
  "provider": "grok2api-proxy",
  "model": "grok-4.20-multi-agent-console",
  "api_url": "http://127.0.0.1:8000/v1",
  "api_key": "..."
}
```

Environment variables take priority:

| Variable | Purpose |
| --- | --- |
| `GROK_API_URL` | OpenAI/Responses-compatible Grok endpoint |
| `GROK_API_KEY` | Grok endpoint key for proxy or custom endpoints |
| `GROK_MODEL` | Default model for proxy or custom endpoints |
| `XAI_API_KEY` | Official xAI API key; when no URL is configured, this defaults the endpoint to `https://api.x.ai/v1` |
| `XAI_API_URL` / `XAI_BASE_URL` | Official xAI-compatible base URL override |
| `XAI_MODEL` | Default official xAI model |
| `TAVILY_API_KEY` | Enables web extraction in `web fetch`, `web map`, web `source fetch`, web `source fetch-all`, and explicit `--extra-sources` |
| `TAVILY_API_URL` | Tavily endpoint |

Check the masked live config:

```bash
grok-search-cli config show
grok-search-cli config set-api-url "https://api.x.ai/v1"
grok-search-cli config set-api-key "xai-..."
grok-search-cli config set-model "grok-4.3"
grok-search-cli config set-tavily-key tvly-...
```

Diagnose the local setup without exposing keys:

```bash
grok-search-cli doctor
grok-search-cli doctor --live-x
```

Tavily and bird are optional. `doctor` reports `tavily_configured`, `bird_available`, and `bird_credentials_ok` so agents can pick the right path.

## Realtime Search

Basic Grok search writes a session artifact:

```bash
grok-search-cli search "latest AI agent browser control research"
```

`search` defaults to a 300 second timeout because Grok realtime and X Search requests can be slow through proxy chains. Use `--timeout-seconds <n>` for a specific run.

For searches that may run for minutes, submit a background job instead of blocking the current terminal:

```bash
grok-search-cli search --background --x-search --timeout-seconds 600 \
  "next week US stock market SPX trend and key levels"
```

The command returns immediately with a `job_id`, `log_file`, and ready-to-run status/result commands. Jobs are stored under `~/.config/grok-search/jobs/`; completed search artifacts still live under `~/.config/grok-search/sessions/`.

```bash
grok-search-cli job status <job_id>
grok-search-cli job result <job_id>
grok-search-cli job wait <job_id>
grok-search-cli job list
grok-search-cli job cancel <job_id>
```

Use `job wait` when an agent runner can keep a separate long-running terminal session open; it polls the job and prints the final search artifact as soon as the job succeeds.

X Search is first-class:

```bash
grok-search-cli search \
  --x-search \
  --recency-days 14 \
  --prefer-recent \
  "latest Grok model discussion on X"
```

Constrain X by handle and date:

```bash
grok-search-cli search \
  --x-search \
  --allowed-x-handle xai \
  --from-date 2026-05-01 \
  --to-date 2026-05-18 \
  "Grok announcements"
```

Enable media understanding for X evidence:

```bash
grok-search-cli search \
  --x-search \
  --enable-image-understanding \
  --enable-video-understanding \
  "recent Grok demo videos from xAI"
```

The text output includes:

```text
session_id: <id>
artifact: ~/.config/grok-search/sessions/<id>.json
sources_count: <n>
tool_calls: <n>
warnings: ...
```

Inspect the durable Source Pack:

```bash
grok-search-cli source index <session_id>
grok-search-cli source links <session_id>
```

`source index` writes `sources-<session_id>/source-index.md` with numbered sources and ready-to-run `source fetch --index N` commands. `source links` writes `sources-<session_id>/links.txt` with all source URLs, including X URLs. Use `--stdout` on either command when terminal output is small enough.

Fetch one selected source after inspecting the index:

```bash
grok-search-cli source fetch <session_id> --index 3
grok-search-cli source fetch <session_id> --url "https://x.com/xai/status/..."
```

`source fetch` writes content under `sources-<session_id>/`: web sources become Markdown from Tavily, X sources become JSON from local authenticated `bird read --json`. Its JSON summary includes `latency_ms` and `bytes` for lightweight fetch performance tracking.

Fetch all artifact sources in parallel only when the agent truly needs a full batch:

```bash
grok-search-cli source fetch-all <session_id> --parallel 8 --chunk-size 10
```

Web sources go through Tavily Extract. X sources go through local authenticated `bird read --json`, with separate X concurrency. If the artifact is X-only, `source fetch-all` does not require a Tavily key; if it contains web sources and Tavily is not configured, use `--no-web`. If bird is not installed or not authenticated, use `--no-x`.

```bash
grok-search-cli source fetch-all <session_id> --parallel 8 --chunk-size 10 --x-parallel 4
```

`source fetch-all` writes `web-000.json`, `x-000-<post_id>.json`, and `manifest.json` under `sources-<session_id>/`. The manifest includes per-chunk `latency_ms` and `bytes`.

## Fetch And Map

Fetch full page content:

```bash
grok-search-cli web fetch "https://docs.x.ai/developers/tools/x-search"
```

Map a site:

```bash
grok-search-cli web map "https://docs.x.ai" --instructions "only tool documentation"
```

These commands use Tavily only. If Tavily is not configured, skip them and use `source index` or `source links` for agentic follow-up through other tools.

## Planning

For complex jobs, emit a command plan before running search:

```bash
grok-search-cli plan "recent X community evidence about Grok 4.20 multi-agent console"
```

## Agent Skill

The repo includes a reusable skill at:

```text
skills/grok-search-cli/SKILL.md
```

Agents should use the skill as a thin orchestration layer. The CLI owns execution, source parsing, artifacts, and evidence contracts.
