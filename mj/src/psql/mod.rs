use clap::Subcommand;
use std::process::ExitCode;

mod better_auth;

#[derive(Subcommand)]
pub(crate) enum PsqlCommand {
    /// Manage better-auth tables (see `mj psql better-auth` for subcommands)
    BetterAuth {
        #[command(subcommand)]
        command: better_auth::BetterAuthCommand,
    },
}

pub(super) fn run(command: PsqlCommand) -> ExitCode {
    match command {
        PsqlCommand::BetterAuth { command } => better_auth::run(command),
    }
}
