use std::env;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) const ENV_VAR: &str = "RENDER_API_KEY";
const APP_DIR: &str = "mj";
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

pub(crate) struct Store {
    path: PathBuf,
    ephemeral: bool,
}

impl Store {
    pub(crate) fn effective_key(&self) -> Option<(String, KeySource)> {
        env_key()
            .map(|k| (k, KeySource::Env))
            .or_else(|| load_key(&self.path).map(|k| (k, KeySource::Stored)))
    }

    pub(crate) fn save(&self, key: &str) -> io::Result<()> {
        save_key(&self.path, key)
    }

    pub(crate) fn clear(&self) -> io::Result<bool> {
        clear_key(&self.path)
    }

    pub(crate) fn retention_hint(&self) -> &'static str {
        if self.ephemeral {
            "It will be used by `mj` until this machine restarts or you run `mj render exit`."
        } else {
            "It will be used by `mj` until you run `mj render exit`."
        }
    }
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

pub(crate) fn default_store() -> Store {
    resolve_store(
        env::var_os("XDG_RUNTIME_DIR").as_deref(),
        env::var_os("XDG_STATE_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
        &env::temp_dir(),
    )
}

fn resolve_store(
    xdg_runtime_dir: Option<&OsStr>,
    xdg_state_home: Option<&OsStr>,
    home: Option<&OsStr>,
    temp_dir: &Path,
) -> Store {
    if let Some(dir) = xdg_runtime_dir
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_dir())
    {
        return Store {
            path: dir.join(STORE_FILE),
            ephemeral: true,
        };
    }
    let state_dir = xdg_state_home
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| PathBuf::from(h).join(".local").join("state")))
        .unwrap_or_else(|| temp_dir.to_path_buf());
    Store {
        path: state_dir.join(APP_DIR).join(STORE_FILE),
        ephemeral: false,
    }
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

fn save_key(path: &Path, key: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = temp_sibling(path)?;
    let result = write_secret_file(&tmp, key).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn write_secret_file(tmp: &Path, key: &str) -> io::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(tmp)?;
    file.write_all(key.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn temp_sibling(path: &Path) -> io::Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let mut name = std::ffi::OsString::from(".");
    name.push(file_name);
    name.push(format!(".tmp.{pid}.{nanos}"));
    Ok(path.parent().unwrap_or(Path::new("")).join(name))
}

fn clear_key(path: &Path) -> io::Result<bool> {
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
        Ok(_) => Validity::Valid,
        Err(ureq::Error::StatusCode(401 | 403)) => Validity::Invalid,
        Err(ureq::Error::StatusCode(code)) => Validity::Unknown(format!(
            "unexpected HTTP status {code} from the Render API"
        )),
        Err(e) => Validity::Unknown(format!("could not reach the Render API: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn home_state_path(home: &Path) -> PathBuf {
        home.join(".local")
            .join("state")
            .join(APP_DIR)
            .join(STORE_FILE)
    }

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
    fn resolve_store_prefers_xdg_runtime_dir_when_valid() {
        let xdg = TempDir::new().unwrap();
        let temp = TempDir::new().unwrap();
        let store = resolve_store(Some(xdg.path().as_os_str()), None, None, temp.path());
        assert_eq!(store.path, xdg.path().join(STORE_FILE));
        assert!(store.ephemeral);
    }

    #[test]
    fn resolve_store_falls_back_to_xdg_state_home() {
        let state = TempDir::new().unwrap();
        let temp = TempDir::new().unwrap();
        let store = resolve_store(None, Some(state.path().as_os_str()), None, temp.path());
        assert_eq!(store.path, state.path().join(APP_DIR).join(STORE_FILE));
        assert!(!store.ephemeral);
    }

    #[test]
    fn resolve_store_falls_back_to_home_local_state() {
        let home = TempDir::new().unwrap();
        let temp = TempDir::new().unwrap();
        let store = resolve_store(None, None, Some(home.path().as_os_str()), temp.path());
        assert_eq!(store.path, home_state_path(home.path()));
        assert!(!store.ephemeral);
    }

    #[test]
    fn resolve_store_ignores_invalid_xdg_runtime_dir() {
        let home = TempDir::new().unwrap();
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join("does-not-exist");

        let store = resolve_store(
            Some(missing.as_os_str()),
            None,
            Some(home.path().as_os_str()),
            temp.path(),
        );
        assert_eq!(store.path, home_state_path(home.path()));
        assert!(!store.ephemeral);

        let store = resolve_store(
            Some(OsStr::new("relative/path")),
            None,
            Some(home.path().as_os_str()),
            temp.path(),
        );
        assert_eq!(store.path, home_state_path(home.path()));
        assert!(!store.ephemeral);
    }

    #[test]
    fn resolve_store_ignores_relative_xdg_state_home() {
        let home = TempDir::new().unwrap();
        let temp = TempDir::new().unwrap();
        let store = resolve_store(
            None,
            Some(OsStr::new("relative/state")),
            Some(home.path().as_os_str()),
            temp.path(),
        );
        assert_eq!(store.path, home_state_path(home.path()));
        assert!(!store.ephemeral);
    }

    #[test]
    fn resolve_store_last_resort_is_temp_dir() {
        let temp = TempDir::new().unwrap();
        let store = resolve_store(None, None, None, temp.path());
        assert_eq!(store.path, temp.path().join(APP_DIR).join(STORE_FILE));
        assert!(!store.ephemeral);
    }

    #[test]
    fn save_key_leaves_no_tmp_siblings_behind() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(STORE_FILE);
        save_key(&path, "rnd_first").unwrap();
        save_key(&path, "rnd_second").unwrap();

        let entries: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![std::ffi::OsString::from(STORE_FILE)]);
        assert_eq!(load_key(&path), Some("rnd_second".to_owned()));
    }

    #[test]
    fn save_key_creates_missing_parent_directories() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("mj").join(STORE_FILE);
        save_key(&path, "rnd_secret").unwrap();
        assert_eq!(load_key(&path), Some("rnd_secret".to_owned()));
    }
}
