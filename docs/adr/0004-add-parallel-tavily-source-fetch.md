# Add parallel Tavily source fetch as a read-layer command

grok-search-cli should provide a one-line read-layer command for fetching non-X web sources from a saved Session Artifact through Tavily Extract:

```bash
grok-search-cli fetch-sources <session_id> --parallel 8 --chunk-size 10
```

This does not mean Grok calls Tavily. Grok still performs source reconnaissance and writes the Source Pack. `fetch-sources` starts after that, reads the artifact, skips X URLs by default, batches ordinary web URLs, and fetches them through Tavily in parallel.

This keeps the agent workflow simple without turning search into provider routing:

- `search` discovers sources through Grok/X Search.
- `sources` inspects or emits source URLs.
- `fetch-sources` quickly reads ordinary web sources through Tavily.
- X sources stay on X-native read tools unless `--include-x` is explicitly used.
