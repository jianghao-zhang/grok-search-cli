# Add parallel source fetch as a read-layer command

grok-search-cli should provide an agentic read workflow for saved Session Artifact sources. The default path is a file-backed source index:

```bash
grok-search-cli source index <session_id>
grok-search-cli source fetch <session_id> --index 3
```

The full-batch path remains available for cases that truly need all source bodies:

```bash
grok-search-cli source fetch-all <session_id> --parallel 8 --chunk-size 10 --x-parallel 4
```

This does not mean Grok calls Tavily or bird. Grok still performs source reconnaissance and writes the Source Pack. `fetch-sources` starts after that, reads the artifact, batches ordinary web URLs through Tavily Extract, and reads X URLs through local authenticated `bird read --json`.

This keeps the agent workflow simple without turning search into provider routing:

- `search` discovers sources through Grok/X Search.
- `source index` emits a durable Source Index instead of dumping large output into the terminal.
- `source links` emits a plain link list for lightweight routing.
- `source fetch --index N` reads one selected source through Tavily or bird-cli.
- `source fetch-all` quickly reads all ordinary web sources through Tavily and all X sources through bird-cli when batch fetching is explicitly useful.
- `--no-web` and `--no-x` exist for explicit narrowing, not as fallback routing.
