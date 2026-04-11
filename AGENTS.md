# Repository Guidelines

## Collaboration Rules
- After finishing a task, leave a concise follow-up question instead of silently ending the conversation.
- If the runtime provides a dedicated user-input tool (such as: `ask_user` ...), prefer it. Otherwise, ask in plain text.
- Do not revert unrelated local changes. This repository may contain in-progress data or generated assets.

## Project Structure
- `src/` contains the Rust CLI and shared business logic used by both CLI and desktop.
- `src/main.rs` is the CLI entrypoint.
- `src/cli.rs`, `src/app.rs`, `src/logging.rs`, and `src/core/` hold argument parsing, orchestration, logging, storage, and execution logic.
- `desktop/` contains the desktop app frontend built with Vite, React, TypeScript, Tailwind CSS, and shadcn-style UI components.
- `desktop/src-tauri/` contains the Tauri shell, Rust commands, packaging config, icons, capabilities, and bundled resources.
- `config/*.toml` and `desktop/src-tauri/resources/bundled.reading.sqlite` are runtime data assets, not library code.
- Generated output lives in `target/`, `desktop/dist/`, `desktop/src-tauri/target/`, and `desktop/node_modules/`; do not edit these manually.

## Build, Test, and Development Commands
Run from the repository root unless noted otherwise.

### Rust workspace
- `cargo check --workspace` checks the shared crate and Tauri shell.
- `cargo test` runs Rust tests for the root crate.
- `cargo fmt --check` verifies Rust formatting with the repo rustfmt settings.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` enforces lint-clean Rust code.
- `cargo run -- --help` shows CLI usage.
- `cargo run -- -c <course_id> -m <manager_id> -f <fc> -n 5` runs one CLI task against five random shops.

### Desktop app
- `cd desktop && npm install` installs frontend dependencies.
- `cd desktop && npm run dev` starts only the Vite frontend dev server on port `1420`.
- `cd desktop && npm run tauri:dev` starts the Tauri desktop app in development mode.
- `cd desktop && npm run build` builds the frontend bundle.
- `cd desktop && npm run tauri:build` builds desktop bundles for the current host platform.
- `cd desktop && npm run tauri:build:mac` builds `.app` and `.dmg` bundles on macOS.
- `cd desktop && npm run tauri:build:win` builds an NSIS installer on Windows.

## Coding Style
- Rust uses edition `2024` with `rustfmt.toml` configured for 2-space indentation.
- Follow standard Rust naming: `snake_case` for functions and variables, `PascalCase` for types, and `SCREAMING_SNAKE_CASE` for constants.
- Prefer `anyhow::Context` for filesystem or network failures so CLI and desktop errors stay actionable.
- Keep reusable business logic in the shared Rust crate. Tauri command handlers should stay thin and delegate to library code.
- Frontend code should stay in TypeScript React function components. Reusable primitives belong in `desktop/src/components/ui/`; feature state and async flows belong in `desktop/src/features/`.
- Preserve the existing Tailwind and component patterns instead of introducing a second styling approach.

## Testing Guidelines
- Add Rust unit tests next to the code they exercise with `#[cfg(test)]` modules.
- Favor tests for pure parsing, loading, selection, and persistence logic over tests that require live HTTP calls.
- For Tauri or frontend changes, at minimum run `cargo check --workspace` and `cd desktop && npm run build`.
- No frontend unit-test runner is configured today. If you introduce one, document the command here and in `README.md`.

## Data and Security Notes
- Treat `config/open_ids.toml`, `config/shop.toml`, `config/province.toml`, and `desktop/src-tauri/resources/bundled.reading.sqlite` as sensitive operational data.
- Do not paste real identifiers, shop data, or database contents into issues, commits, pull requests, or AI prompts unless explicitly required.
- Review diffs to TOML files and bundled SQLite resources carefully before committing.
- The bundled SQLite file is a template for packaged desktop builds; avoid overwriting it casually during development.

## Commit and Pull Request Guidelines
- Use short, imperative commit subjects. Conventional prefixes such as `feat:`, `fix:`, `refactor:`, and `docs:` are preferred.
- Keep commits scoped. Do not mix data-file churn with code changes unless the change requires both.
- Pull requests should summarize user-visible behavior changes, list the commands you ran, and note the platform tested for desktop work.
- Include screenshots or short recordings for UI changes when they materially affect the desktop app.
