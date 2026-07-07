use crate::render::shared::auth;
use std::process::ExitCode;

pub(super) fn cmd_render_exit() -> ExitCode {
    match auth::clear_key(&auth::default_store_path()) {
        Ok(true) => println!("Cleared the saved Render API key."),
        Ok(false) => println!("No saved Render API key to clear."),
        Err(e) => {
            eprintln!("Failed to clear the saved Render API key: {e}");
            return ExitCode::FAILURE;
        }
    }

    auth::warn_env_override(None);

    ExitCode::SUCCESS
}
