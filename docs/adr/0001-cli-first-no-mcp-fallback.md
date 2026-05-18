# Replace Grok Search MCP with grok-search-cli

The product boundary is `grok-search-cli`, a Rust CLI used directly by humans and by agent skills. The old FastMCP server shape is retired and no compatibility MCP adapter is kept in this repository.

The old MCP search surface is migrated into CLI commands: `search`, `sources`, `fetch`, `map`, `models`, `config set-model`, and `plan`. The rejected `toggle_builtin_tools` capability is intentionally omitted because it mutates external agent settings rather than improving search.

Consequences:

- Agents call the CLI through a skill workflow instead of depending on MCP lifecycle state.
- Search sessions are disk-backed artifacts rather than in-memory MCP cache entries.
- The CLI owns testable behavior; skills own lightweight orchestration and follow-up verification choices.
