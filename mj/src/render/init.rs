use crate::render::shared::auth::{self, Validity};
use crate::util::nonempty_trimmed;
use std::io;
use std::process::ExitCode;

pub(super) fn cmd_render_init(provided_key: Option<&str>) -> ExitCode {
    let store = auth::default_store();

    if let Some(key) = provided_key {
        return validate_and_save(&store, key);
    }

    if let Some((existing, source)) = store.effective_key() {
        println!("Found a Render API key ({}).", source.describe());
        match auth::validate_key(&existing) {
            Validity::Valid => {
                println!("It is valid — you're all set.");
                return ExitCode::SUCCESS;
            }
            Validity::Invalid => {
                println!("It is no longer valid. Let's set a new one.");
            }
            Validity::Unknown(reason) => {
                eprintln!("Could not validate the API key: {reason}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        println!("No Render API key found.");
    }

    println!();
    println!("Create a Render API key here:");
    println!("  {}", auth::hyperlink(auth::CREATE_KEY_URL));
    println!();

    let provided = match prompt_secret("Paste your Render API key (input hidden): ") {
        Ok(Some(k)) => k,
        Ok(None) => {
            eprintln!("No key provided.");
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("Failed to read input: {e}");
            return ExitCode::FAILURE;
        }
    };

    validate_and_save(&store, &provided)
}

fn validate_and_save(store: &auth::Store, key: &str) -> ExitCode {
    match auth::validate_key(key) {
        Validity::Valid => {}
        Validity::Invalid => {
            eprintln!("That key was rejected by the Render API. Nothing was saved.");
            return ExitCode::FAILURE;
        }
        Validity::Unknown(reason) => {
            eprintln!("Could not validate the key: {reason}");
            eprintln!("Nothing was saved.");
            return ExitCode::FAILURE;
        }
    }

    if let Err(e) = store.save(key) {
        eprintln!("Failed to save the API key: {e}");
        return ExitCode::FAILURE;
    }

    println!("API key validated and saved.");
    println!("{}", store.retention_hint());

    auth::warn_env_override(Some(key));

    ExitCode::SUCCESS
}

fn prompt_secret(prompt: &str) -> io::Result<Option<String>> {
    match rpassword::prompt_password(prompt) {
        Ok(raw) => Ok(nonempty_trimmed(&raw)),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(e),
    }
}
