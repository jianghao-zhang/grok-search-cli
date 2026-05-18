# Research: Improve grok-search-cli realtime source reconnaissance

## Config
- **Modify**: `Cargo.toml`, `src/**/*.rs`, `README.md`, `skills/grok-search-cli/SKILL.md`, `docs/**/*.md`
- **Measure**: `python3 scripts/measure_cli.py`
- **Direction**: lower
- **Iterations**: 4
- **Budget**: 300

## Metric Integrity Check

1. **Decoupling test**: A change could make the CLI faster by removing JSON fields or skipping config parsing while hurting the true goal. The measure script guards against this by parsing `plan` output, requiring expected fields, running unit tests, and requiring the binary to build.
2. **Determinism test**: Runtime depends on wall-clock noise. The measure script uses median-of-7 for the two hot commands and a deterministic release binary size penalty.
3. **Degenerate-input test**: Emptying or stubbing the CLI would either fail build/tests or fail output-shape checks, producing a large penalty instead of a better score.

## Current State
- Baseline: 346.431
- Target: reduce score without losing realtime/X evidence behavior

## Constraints
- Preserve old Grok Search MCP feature parity except `toggle_builtin_tools`.
- Keep X Search as a first-class source surface, not provider routing.
- Do not require network for the primary Measure command.
- Do not print or persist API keys.
- Keep CLI output stable enough for agent skills.

## Dead Ends (DO NOT RETRY)

| Exp ID | What was tried | Why it failed |
|--------|---------------|---------------|
| — | — | — |

## Cross-Session Learnings

- Session 2026-05-18: Baseline metric should stay offline; live X Search is a smoke test, not the optimization fitness function.

## Past Results

| ID | Hypothesis | Result | Keep? |
|----|------------|--------|-------|
| 0 | baseline | 346.431 | ✓ |
| 1 | strip release symbols and abort panics | 222.037 | ✓ |
| 2 | size-oriented release profile with thin LTO | 173.206 | ✓ |
| 3 | dedupe X internal status URLs by post id | 173.928 | ✓ |
