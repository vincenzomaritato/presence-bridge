# Contributing

## Prerequisites

- Rust stable (latest available).
- Platform-specific runtime (Music.app / Windows media app / MPRIS player).

## Workflow

1. Create a branch (`codex/<short-topic>`).
2. Keep commits small and atomic.
3. Add/update tests for logic changes.
4. Run:
   - `cargo fmt`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
5. Open a PR with:
   - Problem statement
   - Approach and tradeoffs
   - Test evidence

## Coding Guidelines

- Prefer clear, explicit code over cleverness.
- Keep provider-specific behavior behind provider boundaries.
- Avoid adding dependencies without clear value.
- Handle runtime failures gracefully; daemon must stay alive.
