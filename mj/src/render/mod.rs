use clap::Subcommand;
use std::process::ExitCode;

mod exit;
mod init;
mod shared;

#[derive(Subcommand)]
pub(crate) enum RenderCommand {
    /// Save and validate a Render API key
    Init {
        /// Render API key (prompts if omitted)
        #[arg(long, value_name = "KEY", value_parser = crate::util::nonempty_arg)]
        api_key: Option<String>,
    },
    /// Delete the saved Render API key
    Exit,
}

pub(super) fn run(command: RenderCommand) -> ExitCode {
    match command {
        RenderCommand::Init { api_key } => init::cmd_render_init(api_key.as_deref()),
        RenderCommand::Exit => exit::cmd_render_exit(),
    }
}
