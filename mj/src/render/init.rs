use crate::render::shared::auth::{self, Validity};
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

pub(super) fn cmd_render_init() -> ExitCode {
    let store_path = auth::default_store_path();

    if let Some((existing, source)) = auth::effective_key(&store_path) {
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
    println!("  {}", auth::CREATE_KEY_URL);
    println!();

    let provided = match prompt_line("Paste your Render API key: ") {
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

    match auth::validate_key(&provided) {
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

    if let Err(e) = auth::save_key(&store_path, &provided) {
        eprintln!("Failed to save the API key: {e}");
        return ExitCode::FAILURE;
    }

    println!("API key validated and saved.");
    println!("It will be used by `mj` until this machine restarts or you run `mj render exit`.");

    auth::warn_env_override(Some(&provided));

    ExitCode::SUCCESS
}

fn prompt_line(prompt: &str) -> io::Result<Option<String>> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    read_trimmed_line(prompt, &mut reader, &mut writer)
}

fn read_trimmed_line<R: BufRead, W: Write>(
    prompt: &str,
    reader: &mut R,
    writer: &mut W,
) -> io::Result<Option<String>> {
    write!(writer, "{prompt}")?;
    writer.flush()?;
    let mut input = String::new();
    if reader.read_line(&mut input)? == 0 {
        return Ok(None);
    }
    Ok(auth::nonempty_trimmed(input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_trimmed_line_returns_trimmed_value_and_writes_prompt() {
        let mut reader: &[u8] = b"  rnd_abc123  \n";
        let mut writer: Vec<u8> = Vec::new();
        let got = read_trimmed_line("Key: ", &mut reader, &mut writer).unwrap();
        assert_eq!(got, Some("rnd_abc123".to_owned()));
        assert_eq!(String::from_utf8(writer).unwrap(), "Key: ");
    }

    #[test]
    fn read_trimmed_line_returns_none_on_blank_or_eof() {
        for input in ["   \n".as_bytes(), b"" as &[u8]] {
            let mut reader = input;
            let mut writer: Vec<u8> = Vec::new();
            assert_eq!(
                read_trimmed_line("Key: ", &mut reader, &mut writer).unwrap(),
                None
            );
        }
    }
}
