use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod psql;
mod render;
mod util;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage the Render CLI (see `mj render` for subcommands)
    Render {
        #[command(subcommand)]
        command: render::RenderCommand,
    },
    /// Postgres helpers (see `mj psql` for subcommands)
    Psql {
        #[command(subcommand)]
        command: psql::PsqlCommand,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Render { command } => render::run(command),
        Command::Psql { command } => psql::run(command),
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
