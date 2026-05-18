# Grok Search CLI

This context defines the product language for the Grok-centered search tool as it moves from an MCP-first shape to a CLI-first agent workflow.

## Language

**grok-search-cli**:
The canonical product boundary for Grok-centered search commands used by humans and agents.
_Avoid_: Grok Search MCP, smart search router

**Grok Search MCP**:
The retired MCP-server integration shape for Grok Search.
_Avoid_: using it to describe the new primary product

**Skill Workflow**:
An agent-facing orchestration layer that decides when to call `grok-search-cli` commands.
_Avoid_: search engine routing, provider router

**Orchestration Policy**:
The lightweight skill responsibility for choosing the next `grok-search-cli` command and evidence format.
_Avoid_: business logic, source parsing, provider fallback

**Feature Parity**:
The requirement that old Grok Search MCP search capabilities have first-class `grok-search-cli` equivalents.
_Avoid_: partial rewrite, search-only CLI

**Native Web Tool Control**:
The retired capability that modified Claude Code settings to disable built-in web tools.
_Avoid_: toggle_builtin_tools, automatic agent config mutation

**Planning Command**:
An optional CLI command that emits a JSON plan for which `grok-search-cli` commands to run.
_Avoid_: mandatory planning, MCP phase tools

**Session Artifact**:
A disk-backed JSON record created by `grok-search-cli search` for later source retrieval and audit.
_Avoid_: in-memory MCP session cache

**Search Controls**:
Composable CLI flags that tune speed, accuracy, source depth, and output shape per search.
_Avoid_: fixed search profiles, hidden capability tiers

**Evidence Contract**:
The structured output promise that separates answer text, primary sources, extra sources, warnings, attempts, and artifact paths.
_Avoid_: opaque answer blob, merged unverifiable sources

**Source Reconnaissance**:
The core product job of finding high-recall realtime leads, especially from X, community, and edge sources.
_Avoid_: generic web search replacement

**Source Pack**:
A structured collection of candidate sources with metadata, provenance, timestamps, and verification status.
_Avoid_: flat URL list

**Temporal Evidence Weighting**:
The rule that newer evidence carries higher default weight when a domain changes quickly or sources conflict.
_Avoid_: timeless ranking, stale SOTA assumptions

**X Search Surface**:
The Grok/xAI server-side search surface for X-native keyword search, semantic search, user search, and thread fetch.
_Avoid_: generic Twitter scraping, provider routing

**X Community Reconnaissance**:
The `grok-search-cli` search mode for realtime X community evidence: posts, threads, handles, media-bearing posts, and cited X URLs.
_Avoid_: social sentiment fluff, unverifiable community summary

**X Search Controls**:
Composable CLI controls for X-specific scope: date windows, allowed or excluded handles, image understanding, video understanding, and thread fetch.
_Avoid_: fixed social-search presets, hidden X filters

**Citation Ledger**:
The auditable record of all source URLs and inline citations returned by Grok tool-enabled requests.
_Avoid_: answer-only citations, lost tool provenance

**Source Fetch Command**:
A CLI read-layer command that takes a Session Artifact and fetches non-X web sources through Tavily in parallel.
_Avoid_: Grok calling Tavily, hidden provider routing

## Relationships

- **grok-search-cli** is the primary product boundary.
- **Skill Workflow** calls **grok-search-cli**; it does not own search execution.
- **Orchestration Policy** belongs in **Skill Workflow**; testable search behavior belongs in **grok-search-cli**.
- **Feature Parity** preserves search capabilities while replacing the MCP surface with CLI commands.
- **Grok Search MCP** is retired rather than kept as a compatibility fallback.
- **Native Web Tool Control** is not part of **Feature Parity**.
- **Planning Command** replaces the old MCP phase tools without requiring planning before simple searches.
- A **Session Artifact** stores search output and sources across CLI invocations.
- **Search Controls** provide fast defaults without limiting what a single search can request.
- The **Evidence Contract** makes accuracy auditable without turning `grok-search-cli` into a multi-engine router.
- **Source Reconnaissance** is the primary value of **grok-search-cli**; generic web search remains outside its boundary.
- A **Source Pack** is the main artifact produced by **Source Reconnaissance**.
- **Temporal Evidence Weighting** influences source ordering and conflict handling in realtime domains.
- **X Search Surface** is a first-class Grok-native surface inside **Source Reconnaissance**, not a route to another search provider.
- **X Community Reconnaissance** uses **X Search Controls** to preserve recall, speed, and auditability without fixed profiles.
- A **Citation Ledger** feeds the **Evidence Contract** and **Session Artifact** so later agents can verify or fetch original X and web sources.
- **Source Fetch Command** is allowed because it starts from an existing **Session Artifact** and belongs to the read/verification step, not Grok search execution.
