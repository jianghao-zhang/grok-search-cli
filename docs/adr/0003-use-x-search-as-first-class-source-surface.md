# Use X Search as a first-class source surface

grok-search-cli treats Grok/xAI X Search as a first-class Source Reconnaissance surface, not as a generic provider route. The CLI exposes X-specific controls for time windows, allowed or excluded handles, image understanding, video understanding, and thread-oriented retrieval while keeping the skill workflow responsible only for lightweight orchestration.

This decision follows the current xAI tool shape: X Search supports keyword search, semantic search, user search, and thread fetch on X, with date range and handle filters plus image and video understanding. These controls match the product's realtime evidence goal better than a broad smart-search router.

Consequences:

- `grok-search-cli search --x-search` must preserve `custom_tool_call` records such as `x_keyword_search`.
- X results enter the Source Pack with X-native metadata where available: post URL, handle, post id, source mode, media flags, and citation provenance.
- Newer X evidence receives Temporal Evidence Weighting in fast-moving domains, while conflicts remain visible in the Evidence Contract.
- The CLI preserves a Citation Ledger for source URLs encountered by tool-enabled Grok requests, not only URLs cited in final prose.
- External tools such as Chrome, Exa, Tavily, or Codex Web Search remain verification or follow-up read paths owned by the agent workflow, not embedded search routes inside `grok-search-cli`.
