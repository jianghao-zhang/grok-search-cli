# grok-search-cli

Rust CLI for Grok-centered realtime source reconnaissance.

`grok-search-cli` is not a generic smart-search router and not an MCP server. It is a fast CLI surface for using Grok where Grok is strongest: realtime X/community discovery, fresh source gathering, and high-recall lead finding in domains where yesterday's answer may already be stale.

The main artifact is a disk-backed Source Pack: answer text, sources, X URLs, tool calls, warnings, timestamps, and citation provenance saved as JSON for later agent verification.

## Why This Exists

Grok is unusually useful for realtime and X-adjacent discovery. The goal here is to amplify that long board instead of replacing Codex Web Search, Exa, Tavily, Chrome, or browser automation.

Use it when recency and community evidence matter:

- X launch-day reactions, incidents, rumors, and public threads
- AI research/model/tooling changes where newer evidence should carry more weight
- obscure community sources that broader web search often misses
- Grok-native source gathering before another agent verifies original pages or posts

## Install

```bash
cargo install --git https://github.com/jianghao-zhang/grok-search-cli.git
```

For local development:

```bash
cargo build --release
```

## Configuration

The CLI reads the same local config surface used by the old Grok Search setup:

```json
{
  "model": "grok-4.20-multi-agent-console",
  "api_url": "http://127.0.0.1:8000/v1",
  "api_key": "..."
}
```

Environment variables take priority:

| Variable | Purpose |
| --- | --- |
| `GROK_API_URL` | OpenAI/Responses-compatible Grok endpoint |
| `GROK_API_KEY` | Grok endpoint key |
| `GROK_MODEL` | Default model |
| `TAVILY_API_KEY` | Enables web extraction in `web fetch`, `web map`, web `source fetch`, web `source fetch-all`, and explicit `--extra-sources` |
| `TAVILY_API_URL` | Tavily endpoint |

Check the masked live config:

```bash
grok-search-cli config show
grok-search-cli config set-tavily-key tvly-...
```

Diagnose the local setup without exposing keys:

```bash
grok-search-cli doctor
grok-search-cli doctor --live-x
```

Tavily and bird are optional. Without Tavily, search, artifacts, source indexes, and link lists still work; only web body extraction is unavailable. Without bird, X Search and X links still work; only local X body fetching is unavailable. `doctor` reports `tavily_configured`, `bird_available`, and `bird_credentials_ok` so agents can pick the right path.

## Realtime Search

Basic Grok search writes a session artifact:

```bash
grok-search-cli search "latest AI agent browser control research"
```

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

`source fetch-all` writes `web-000.json`, `x-000-<post_id>.json`, and `manifest.json` under `sources-<session_id>/`. The manifest includes per-chunk `latency_ms` and `bytes`. Legacy top-level commands such as `sources`, `fetch-source`, `fetch-sources`, `fetch`, and `map` remain available, but new docs use grouped commands.

## Feature Parity With Old Grok Search MCP

The old MCP server exposed search, source retrieval, fetch, map, config diagnostics, model switching, and planning. Those map to CLI commands:

| Old MCP tool | CLI equivalent |
| --- | --- |
| `web_search` | `grok-search-cli search` |
| `get_sources` | `grok-search-cli source index <session_id>` |
| `web_fetch` | `grok-search-cli web fetch <url>` |
| `web_map` | `grok-search-cli web map <url>` |
| `get_config_info` | `grok-search-cli config show` and `grok-search-cli models` |
| `switch_model` | `grok-search-cli config set-model <model>` |
| `plan_*` tools | `grok-search-cli plan "<query>"` |

`grok-search-cli doctor --live-x` is new: it verifies that the current endpoint can actually trigger X Search and returns tool-call/source counts without printing secrets.

`toggle_builtin_tools` is intentionally not migrated. It mutated Claude Code settings and was not a search capability.

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

For complex jobs, emit a command plan without creating MCP planning sessions:

```bash
grok-search-cli plan "recent X community evidence about Grok 4.20 multi-agent console"
```

## Agent Skill

The repo includes a reusable skill at:

```text
skills/grok-search-cli/SKILL.md
```

Agents should use the skill as a thin orchestration layer. The CLI owns execution, source parsing, artifacts, and evidence contracts.
