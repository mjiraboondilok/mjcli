use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) const ENV_VAR: &str = "RENDER_API_KEY";
const STORE_FILE: &str = "mj-render-api-key";
pub(crate) const CREATE_KEY_URL: &str = "https://dashboard.render.com/u/settings?add-api-key";
const API_OWNERS_URL: &str = "https://api.render.com/v1/owners?limit=1";

#[derive(Clone, Copy)]
pub(crate) enum KeySource {
    Env,
    Stored,
}

impl KeySource {
    pub(crate) fn describe(self) -> &'static str {
        match self {
            KeySource::Env => "from the RENDER_API_KEY environment variable",
            KeySource::Stored => "saved by a previous `mj render init`",
        }
    }
}

pub(crate) enum Validity {
    Valid,
    Invalid,
    Unknown(String),
}

pub(crate) fn effective_key(store_path: &Path) -> Option<(String, KeySource)> {
    env_key()
        .map(|k| (k, KeySource::Env))
        .or_else(|| load_key(store_path).map(|k| (k, KeySource::Stored)))
}

pub(crate) fn env_key() -> Option<String> {
    env::var_os(ENV_VAR)
        .and_then(|s| s.into_string().ok())
        .and_then(nonempty_trimmed)
}

pub(crate) fn nonempty_trimmed(s: String) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(crate) fn default_store_path() -> PathBuf {
    env::temp_dir().join(STORE_FILE)
}

pub(crate) fn warn_env_override(just_saved: Option<&str>) {
    let Some(env) = env_key() else { return };
    let consequence = match just_saved {
        Some(saved) if env == saved => return,
        Some(_) => " over the key you just saved",
        None => "",
    };
    println!();
    println!("note: {ENV_VAR} is exported in your shell and takes precedence{consequence}.");
    println!("`mj` can't unset it for you — remove it with:");
    println!("  unset {ENV_VAR}");
}

fn load_key(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(nonempty_trimmed)
}

pub(crate) fn save_key(path: &Path, key: &str) -> io::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(key.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn clear_key(path: &Path) -> io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

pub(crate) fn validate_key(key: &str) -> Validity {
    let result = ureq::get(API_OWNERS_URL)
        .config()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .header("Authorization", format!("Bearer {key}"))
        .header("Accept", "application/json")
        .call();
    match result {
        Ok(resp) => classify_status(resp.status().as_u16()),
        Err(ureq::Error::StatusCode(code)) => classify_status(code),
        Err(e) => Validity::Unknown(format!("could not reach the Render API: {e}")),
    }
}

fn classify_status(code: u16) -> Validity {
    match code {
        200 => Validity::Valid,
        401 | 403 => Validity::Invalid,
        other => Validity::Unknown(format!(
            "unexpected HTTP status {other} from the Render API"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn nonempty_trimmed_trims_and_rejects_empty() {
        assert_eq!(
            nonempty_trimmed("  rnd_abc  ".to_owned()),
            Some("rnd_abc".to_owned())
        );
        assert_eq!(nonempty_trimmed("   ".to_owned()), None);
        assert_eq!(nonempty_trimmed(String::new()), None);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STORE_FILE);
        save_key(&path, "rnd_secret").unwrap();
        assert_eq!(load_key(&path), Some("rnd_secret".to_owned()));
    }

    #[test]
    fn load_key_trims_trailing_newline() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STORE_FILE);
        fs::write(&path, b"rnd_secret\n").unwrap();
        assert_eq!(load_key(&path), Some("rnd_secret".to_owned()));
    }

    #[test]
    fn load_key_is_none_for_missing_or_blank() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join(STORE_FILE);
        assert_eq!(load_key(&missing), None);

        let blank = dir.path().join("blank");
        fs::write(&blank, b"   \n").unwrap();
        assert_eq!(load_key(&blank), None);
    }

    #[test]
    fn clear_key_reports_whether_file_existed() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STORE_FILE);
        assert!(!clear_key(&path).unwrap());

        save_key(&path, "rnd_secret").unwrap();
        assert!(clear_key(&path).unwrap());
        assert_eq!(load_key(&path), None);
    }

    #[cfg(unix)]
    #[test]
    fn save_key_writes_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STORE_FILE);
        fs::write(&path, b"old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        save_key(&path, "rnd_secret").unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn classify_status_maps_codes() {
        assert!(matches!(classify_status(200), Validity::Valid));
        assert!(matches!(classify_status(401), Validity::Invalid));
        assert!(matches!(classify_status(403), Validity::Invalid));
        assert!(matches!(classify_status(500), Validity::Unknown(_)));
    }
}
