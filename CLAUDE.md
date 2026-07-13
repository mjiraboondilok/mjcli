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

`mj` is a small multi-command CLI. Argument parsing and dispatch use `clap`'s derive API
(the `clap` dependency with its `derive` feature).

**Command dispatch is a nested `#[derive(Subcommand)]` tree.** `main.rs` defines the top-level
`Cli` (`#[derive(Parser)]`) whose `Command` enum has one variant per top-level command (`render`,
`psql`); each variant embeds that module's own `Subcommand` enum, nesting one level deeper
(`psql better-auth <check|init|insert>`). Each command module owns its subcommand enum plus a
`run()` function that matches the parsed variant and returns `std::process::ExitCode`. clap
generates `--help`/`--version` and all usage/error output; usage errors exit with clap's
conventional code `2`. Flags whose values must be non-empty use the shared `nonempty_arg` value
parser in `src/util.rs` (which also holds `nonempty_trimmed`). Adding a subcommand means adding a
variant to the appropriate enum plus a match arm in that module's `run()`.

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
  - `psql better-auth check` / `init` / `insert` shell out to the system `psql` binary (not a
    Postgres driver crate): `check` verifies the four core better-auth tables (`user`, `session`,
    `account`, `verification`) exist, `init` creates them, and `insert` creates a user with a
    credential account (scrypt password hash matching better-auth's defaults). The
    `CREATE TABLE IF NOT EXISTS` DDL lives inline in `src/psql/better_auth.rs`, so the table
    declaration order matters for foreign keys (`user` must precede `session`/`account`, which is
    verified by a test).
  - `psql` exit code `2` specifically means "could not connect" (vs. a query/syntax error) —
    `run_psql` uses this to distinguish connection failures from query failures and print the
    right hint (env vars, `~/.pgpass`, or `--connection <URL>`).

When adding a new top-level command or subcommand, follow the existing shape: a `Subcommand` enum
variant, a `run()` dispatcher returning `ExitCode`, and shared logic factored into a `shared/`
module if more than one subcommand needs it (see `render/shared/`).
