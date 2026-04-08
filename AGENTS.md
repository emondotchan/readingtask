# Repository Guidelines

## Dev Instructions
 - **MUST** use `ask_user` and samiliar tools after finished any task. 
 - **NOT ALLOW** auto end conversation.

## Project Structure & Module Organization
`src/main.rs` contains the CLI entrypoint, while parsing and business logic can be split into sibling modules under `src/`. Treat `config/open_ids.toml`, `config/shop.toml`, and `config/province.toml` as runtime data assets, not library code. Keep one-off helpers in `scripts/`; `scripts/format_openid.py` is the only utility script today. Build output lives in `target/` and should not be edited manually.

## Build, Test, and Development Commands
Run from the repository root:

- `cargo run -- --help` shows CLI options for `reading_task`.
- `cargo run -- -c <course_id> -m <manager_id> -f <fc> -n 5` runs the tool against five random shops for one FC.
- `cargo test` executes the test suite. The project currently has no unit tests, so this is mainly a compile and smoke check.
- `cargo fmt --check` verifies formatting with the repo’s Rustfmt settings.
- `cargo clippy --all-targets --all-features -- -D warnings` enforces lint-clean Rust code before review.

## Coding Style & Naming Conventions
This project uses Rust 2024 edition with `rustfmt.toml` set to 2-space indentation. Follow existing Rust conventions: `snake_case` for functions and variables, `PascalCase` for structs, and explicit `serde(rename = "...")` attributes when mapping external field names. Keep CLI-facing errors actionable, and prefer `anyhow::Context` when wrapping file or network failures.

## Testing Guidelines
Add unit tests next to the code they cover with `#[cfg(test)]` blocks in `src/main.rs` unless the code is split into modules later. Name tests for observable behavior, for example `load_open_ids_deduplicates_values`. For network-related changes, isolate pure parsing and selection logic so it can be tested without live HTTP requests.

## Commit & Pull Request Guidelines
This repository has no commit history yet, so there is no established convention to copy. Start with short, imperative commit subjects such as `feat: add shop code filter` or `fix: reject zero count`. Pull requests should describe the user-visible behavior change, list the commands you ran, and include sample CLI input when flags, data files, or request payload logic change.

## Data & Security Notes
`config/open_ids.toml` and the shop datasets contain operational data. Avoid pasting real identifiers into issues or PR descriptions, and review any TOML diffs carefully before merging.
