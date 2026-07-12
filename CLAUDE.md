# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository layout

This is a Cargo workspace defined at the repo root (`dist-workspace.toml`), but the actual crate
lives entirely under `mj/` — that's where `Cargo.toml`, `src/`, and all Rust code are. Run all
`cargo` commands from `mj/`, not the repo root.

- `mj/` — the `mj` CLI crate (all source code)
- `Dockerfile`, `.github/workflows/` — build/release plumbing at the repo root
- `dist-workspace.toml` — config for `cargo-dist`, which builds and publishes GitHub releases

## Common commands

Run from the `mj/` directory:

```sh
cargo build                 # build
cargo run -- <args>         # run, e.g. cargo run -- psql better-auth check
cargo test                  # run all tests
cargo test <test_name>      # run a single test (tests live inline in each module's `#[cfg(test)] mod tests`)
cargo fmt --check --all     # formatting check (CI runs this)
cargo clippy                # lint (CI runs this)
```

CI (`.github/workflows/ci.yml`) runs `cargo fmt --check --all`, `cargo clippy`, and `cargo test`,
all with `working-directory: mj`.

## Architecture

`mj` is a small multi-command CLI with no external command-line-parsing crate — argument parsing
is hand-rolled in `src/args.rs`.

**Command dispatch is a two-level match tree.** `main.rs` matches the top-level command name
(`render`, `psql`) and forwards the remaining args to that command's module, which matches on the
subcommand name and forwards again. Each subcommand module owns its own `print_usage()` and
returns `std::process::ExitCode`. Adding a new subcommand means adding a match arm at the
appropriate level and updating that level's `print_usage()`.

**Two independent command trees currently exist:**

- `render` (`src/render/`) — manages a locally-stored Render API key.
  - `render init` validates a provided or existing key against the Render API and saves it;
    `render exit` deletes it.
  - `src/render/shared/auth.rs` holds the key storage/validation logic shared by both
    subcommands: it resolves an XDG-appropriate storage path (`XDG_RUNTIME_DIR` >
    `XDG_STATE_HOME` > `~/.local/state` > temp dir, tracking whether the location is ephemeral),
    reads `RENDER_API_KEY` as an override that always takes precedence over the stored key, and
    validates keys via a live call to the Render API (`ureq`, 10s timeout).
  - Key files are written via `tempfile` + atomic rename with owner-only (`0600`) permissions.

- `psql` (`src/psql/`) — helpers for Postgres schemas managed by `better-auth`.
  - `psql better-auth check` / `init` shell out to the system `psql` binary (not a Postgres
    driver crate) to check for or create the four core better-auth tables (`user`, `session`,
    `account`, `verification`). The `CREATE TABLE IF NOT EXISTS` DDL lives inline in
    `src/psql/better_auth.rs`, so the table declaration order matters for foreign keys (`user`
    must precede `session`/`account`, which is verified by a test).
  - `psql` exit code `2` specifically means "could not connect" (vs. a query/syntax error) —
    `run_psql` uses this to distinguish connection failures from query failures and print the
    right hint (env vars, `~/.pgpass`, or `--connection <URL>`).

When adding a new top-level command or subcommand, follow the existing shape: a `cmd_*` function
returning `ExitCode`, a local `print_usage()`, and shared logic factored into a `shared/` module
if more than one subcommand needs it (see `render/shared/`).
