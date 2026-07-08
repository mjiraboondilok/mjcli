use crate::render::shared::auth::{self, Validity};
use std::io;
use std::process::ExitCode;

pub(super) fn cmd_render_init(args: &[String]) -> ExitCode {
    let provided_key = match parse_args(args) {
        Ok(key) => key,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("Usage: mj render init [--api-key <KEY>]");
            return ExitCode::FAILURE;
        }
    };

    let store = auth::default_store();

    if let Some(key) = provided_key {
        return validate_and_save(&store, &key);
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

fn parse_args(args: &[String]) -> Result<Option<String>, String> {
    let mut rest = args.iter();
    let mut key: Option<String> = None;

    while let Some(arg) = rest.next() {
        let raw = if arg == "--api-key" {
            rest.next()
                .ok_or_else(|| "error: --api-key requires a value".to_owned())?
                .as_str()
        } else if let Some(value) = arg.strip_prefix("--api-key=") {
            value
        } else {
            return Err(format!("error: unexpected argument '{arg}'"));
        };

        if key.is_some() {
            return Err("error: --api-key given more than once".to_owned());
        }
        key = Some(
            auth::nonempty_trimmed(raw)
                .ok_or_else(|| "error: --api-key value is empty".to_owned())?,
        );
    }

    Ok(key)
}

fn prompt_secret(prompt: &str) -> io::Result<Option<String>> {
    match rpassword::prompt_password(prompt) {
        Ok(raw) => Ok(auth::nonempty_trimmed(&raw)),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<String>, String> {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse_args(&owned)
    }

    #[test]
    fn no_args_yields_no_key() {
        assert_eq!(parse(&[]), Ok(None));
    }

    #[test]
    fn separate_and_inline_forms_both_parse() {
        assert_eq!(parse(&["--api-key", "rnd_abc"]), Ok(Some("rnd_abc".into())));
        assert_eq!(parse(&["--api-key=rnd_abc"]), Ok(Some("rnd_abc".into())));
    }

    #[test]
    fn value_is_trimmed() {
        assert_eq!(parse(&["--api-key", "  rnd_abc  "]), Ok(Some("rnd_abc".into())));
    }

    #[test]
    fn missing_value_is_an_error() {
        assert!(parse(&["--api-key"]).is_err());
    }

    #[test]
    fn empty_value_is_an_error() {
        assert!(parse(&["--api-key="]).is_err());
        assert!(parse(&["--api-key", "   "]).is_err());
    }

    #[test]
    fn duplicate_flag_is_an_error() {
        assert!(parse(&["--api-key", "a", "--api-key", "b"]).is_err());
    }

    #[test]
    fn unexpected_argument_is_an_error() {
        assert!(parse(&["--nope"]).is_err());
        assert!(parse(&["rnd_abc"]).is_err());
    }
}
