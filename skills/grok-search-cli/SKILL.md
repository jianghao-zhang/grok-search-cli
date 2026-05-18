# grok-search-cli

Use this skill when a user or agent needs Grok-centered realtime source reconnaissance, especially for fast-moving topics, X community evidence, launch-day reports, AI research updates, model availability, or edge-source discovery.

The tool is a CLI workflow. Call `grok-search-cli` directly and use its disk-backed session artifacts for follow-up source inspection.

## Routing

Use `grok-search-cli search` when the task needs realtime leads or Grok's X/community strength.

Use `--x-search` when the question depends on X posts, X community reaction, public handles, threads, launches, incidents, rumors, or fresh social evidence.

The `search` command defaults to a 300 second timeout. Broad realtime/X/community queries can run for several minutes with no terminal output, especially through proxy endpoints. Do not treat 30-90 seconds of silence as failure; wait for completion unless the user interrupts or a shorter bound is explicitly needed. Add `--timeout-seconds <n>` only when the task needs a shorter or longer bound.

Use `--from-date`, `--to-date`, or `--recency-days` for fast-changing domains. Prefer recent evidence when sources conflict, but keep the conflict visible.

Use `grok-search-cli source index <session_id>` after a search when the answer is interesting, suspicious, or needs original-source verification.

Prefer `grok-search-cli source index <session_id>` for agentic search. This writes a compact source index with numbered links and per-source fetch commands, so the agent can inspect links first and fetch only the sources that matter.

Use `grok-search-cli source links <session_id>` when the next step only needs a plain link list.

Use `grok-search-cli source fetch <session_id> --index N` to read one selected source through its native reader: Tavily for web, `bird read --json` for X.

Use `grok-search-cli source fetch-all <session_id> --parallel 8 --x-parallel 4` only when the next step truly needs all artifact bodies. Web sources go through Tavily Extract; X sources go through local authenticated `bird read --json`. Use `--no-web` when Tavily is not configured, and `--no-x` when bird is unavailable.

Use `grok-search-cli config set-tavily-key <key>` once if Tavily is not configured and the user has explicitly provided a key.

Use `grok-search-cli web fetch <url>` for full-page extraction and `grok-search-cli web map <url>` for site mapping. These require Tavily.

Use `grok-search-cli plan "<query>"` only for complex searches where a visible command plan helps; do not require planning before simple searches.

Use `grok-search-cli doctor` for local setup diagnostics, including Tavily and bird availability. Use `grok-search-cli doctor --live-x` when the important question is whether this machine's current Grok endpoint can actually trigger X Search.

Use `grok-search-cli config use-official` for the official xAI API. It targets `https://api.x.ai/v1` and defaults to `grok-4.3`; users can supply credentials through `XAI_API_KEY`.

Use `grok-search-cli config use-proxy` for Grok2API or another OpenAI-compatible proxy. It defaults to `http://127.0.0.1:8000/v1` and the local proxy model.

## Common Commands

```bash
grok-search-cli config use-official
grok-search-cli config use-proxy --api-url "http://127.0.0.1:8000/v1"
grok-search-cli search --x-search --recency-days 14 --prefer-recent "latest Grok model discussion on X"
grok-search-cli search --x-search --allowed-x-handle xai --from-date 2026-05-01 "Grok announcements"
grok-search-cli source index <session_id>
grok-search-cli source links <session_id>
grok-search-cli source fetch <session_id> --index 3
grok-search-cli source fetch-all <session_id> --parallel 8 --chunk-size 10
grok-search-cli web fetch "https://example.com/page"
grok-search-cli web map "https://docs.example.com" --instructions "only API reference pages"
grok-search-cli models
grok-search-cli config show
grok-search-cli doctor --live-x
```

## Output Contract

Treat `session_id`, `artifact`, `sources_count`, `tool_calls`, and `warnings` as first-class evidence. The artifact JSON is the durable source pack and citation ledger.

When X Search is used, check for `tool_calls` names such as `x_keyword_search`, `x_semantic_search`, `x_user_search`, or `x_thread_fetch`. Do not rely only on top-level `citations`; some Grok/proxy paths expose X URLs through output text annotations instead.
