use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const RENDER_BIN: &str = "render";
const RENDER_INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/render-oss/cli/refs/heads/main/bin/install.sh";

pub fn cmd_render(args: &[String]) -> ExitCode {
    let Some(sub) = args.first() else {
        print_usage();
        return ExitCode::from(1);
    };

    match sub.as_str() {
        "init" => cmd_render_init(),
        "-h" | "--help" | "help" => {
            print_usage();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("mj render: unknown subcommand '{other}'");
            print_usage();
            ExitCode::from(1)
        }
    }
}

fn print_usage() {
    println!("Usage: mj render <subcommand>");
    println!();
    println!("Subcommands:");
    println!("  init    Ensure the Render CLI is installed and initialized");
}

fn cmd_render_init() -> ExitCode {
    let augmented_path = if exists(RENDER_BIN) {
        println!("Render CLI is installed.");
        None
    } else {
        println!("The Render CLI (`{RENDER_BIN}`) is not installed.");
        if !prompt_accepted("Would you like to install it now?") {
            println!("Skipping installation.");
            return ExitCode::SUCCESS;
        }
        match install_render() {
            Ok(p) => {
                println!("Render CLI installed.");
                Some(p)
            }
            Err(e) => {
                eprintln!("Installation failed: {e}");
                return ExitCode::FAILURE;
            }
        }
    };

    if render_is_initialized(
        env::var_os("RENDER_CLI_CONFIG_PATH").as_deref(),
        env::var_os("HOME").as_deref(),
    ) {
        println!("Render CLI is already initialized.");
        return ExitCode::SUCCESS;
    }

    println!("The Render CLI is not initialized.");
    if !prompt_accepted("Would you like to initialize it now (runs `render login`)?") {
        println!("Skipping initialization.");
        return ExitCode::SUCCESS;
    }

    if let Err(e) = initialize_render(augmented_path.as_deref()) {
        eprintln!("Initialization failed: {e}");
        return ExitCode::FAILURE;
    }
    println!("Render CLI initialized.");
    ExitCode::SUCCESS
}

fn render_is_initialized(config_override: Option<&OsStr>, home: Option<&OsStr>) -> bool {
    find_render_config_path(config_override, home).is_some_and(|p| p.exists())
}

fn find_render_config_path(
    config_override: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Option<PathBuf> {
    if let Some(custom) = config_override {
        return Some(PathBuf::from(custom));
    }
    home.map(|h| {
        PathBuf::from(h)
            .join(".config")
            .join("render")
            .join("config.json")
    })
}

fn install_render() -> io::Result<OsString> {
    if !exists("curl") {
        return Err(io::Error::other(
            "`curl` is required to run the Render CLI install script. Install curl and \
             try again.",
        ));
    }

    let original_path = env::var_os("PATH").unwrap_or_default();

    println!("Installing Render CLI via {RENDER_INSTALL_SCRIPT_URL}...");
    let script = format!("curl -fsSL {RENDER_INSTALL_SCRIPT_URL} | sh");
    let status = Command::new("sh").arg("-c").arg(&script).status()?;
    if !status.success() {
        return Err(io::Error::other(
            "the Render CLI install script exited with a non-zero status",
        ));
    }

    let install_dirs = list_install_dirs(env::var_os("HOME").as_deref());
    let found_dir = find_render_binary_dir(&install_dirs);

    if let Some(binary_dir) = &found_dir
        && !dir_in_path(binary_dir, &original_path)
    {
        eprintln!(
            "note: `{RENDER_BIN}` was installed to {} but that directory is not on your \
             shell's PATH. Add it to your shell startup file, e.g.:",
            binary_dir.display()
        );
        eprintln!(
            "  echo 'export PATH=\"{}:$PATH\"' >> ~/.bashrc   # or ~/.zshrc",
            binary_dir.display()
        );
    }

    if found_dir.is_none() && !exists_in_path(RENDER_BIN, &original_path) {
        return Err(io::Error::other(format!(
            "`{RENDER_BIN}` was not found on PATH after install. You may need to restart your shell."
        )));
    }

    Ok(prepend_to_path(&install_dirs, &original_path).unwrap_or(original_path))
}

fn list_install_dirs(home: Option<&OsStr>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(h) = home {
        dirs.push(PathBuf::from(h).join(".local").join("bin"));
    }
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

fn prepend_to_path(new_dirs: &[PathBuf], current: &OsStr) -> Option<OsString> {
    env::join_paths(new_dirs.iter().cloned().chain(env::split_paths(current))).ok()
}

fn dir_in_path(dir: &Path, path_env: &OsStr) -> bool {
    env::split_paths(path_env).any(|d| d == dir)
}

fn find_render_binary_dir(dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().find(|d| d.join(RENDER_BIN).is_file()).cloned()
}

fn initialize_render(path: Option<&OsStr>) -> io::Result<()> {
    let mut cmd = Command::new(RENDER_BIN);
    if let Some(p) = path {
        cmd.env("PATH", p);
    }
    let status = cmd.arg("login").status()?;
    if !status.success() {
        return Err(io::Error::other(
            "`render login` exited with a non-zero status",
        ));
    }
    Ok(())
}

fn exists_in_path(program: &str, path_env: &OsStr) -> bool {
    env::split_paths(path_env).any(|dir| dir.join(program).is_file())
}

fn exists(program: &str) -> bool {
    env::var_os("PATH").is_some_and(|p| exists_in_path(program, &p))
}

fn prompt_accepted(question: &str) -> bool {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    yes_no_accepted(question, &mut reader, &mut writer).unwrap_or(false)
}

fn yes_no_accepted<R: BufRead, W: Write>(
    question: &str,
    reader: &mut R,
    writer: &mut W,
) -> io::Result<bool> {
    write!(writer, "{question} [y/N]: ")?;
    writer.flush()?;
    let mut input = String::new();
    reader.read_line(&mut input)?;
    let trimmed = input.trim();
    Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn find_render_config_path_uses_override_when_set() {
        let path = find_render_config_path(
            Some(OsStr::new("/custom/render.json")),
            Some(OsStr::new("/home/anyone")),
        );
        assert_eq!(path, Some(PathBuf::from("/custom/render.json")));
    }

    #[test]
    fn find_render_config_path_falls_back_to_home_default() {
        let path = find_render_config_path(None, Some(OsStr::new("/home/mag")));
        assert_eq!(
            path,
            Some(PathBuf::from("/home/mag/.config/render/config.json"))
        );
    }

    #[test]
    fn find_render_config_path_is_none_without_home_or_override() {
        assert_eq!(find_render_config_path(None, None), None);
    }

    #[test]
    fn exists_in_path_finds_program_across_entries() {
        let empty = TempDir::new().unwrap();
        let bindir = TempDir::new().unwrap();
        let bin = bindir.path().join("myprog");
        fs::write(&bin, b"").unwrap();

        let joined = env::join_paths([empty.path(), bindir.path()]).unwrap();
        assert!(exists_in_path("myprog", &joined));
    }

    #[test]
    fn exists_in_path_returns_false_when_missing() {
        let dir = TempDir::new().unwrap();
        let joined = env::join_paths([dir.path()]).unwrap();
        assert!(!exists_in_path("nope-not-here", &joined));
    }

    #[test]
    fn exists_in_path_ignores_directory_matches() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("myprog")).unwrap();
        let joined = env::join_paths([dir.path()]).unwrap();
        assert!(!exists_in_path("myprog", &joined));
    }

    #[test]
    fn yes_no_accepted_accepts_y_and_writes_prompt() {
        let mut reader: &[u8] = b"y\n";
        let mut writer: Vec<u8> = Vec::new();
        assert!(yes_no_accepted("go?", &mut reader, &mut writer).unwrap());
        let prompt = String::from_utf8(writer).unwrap();
        assert!(
            prompt.contains("go?") && prompt.contains("[y/N]"),
            "unexpected prompt: {:?}",
            prompt
        );
    }

    #[test]
    fn yes_no_accepted_accepts_uppercase_and_full_word() {
        for input in ["Y\n", "yes\n", "YES\n", "Yes\n"] {
            let mut reader = input.as_bytes();
            let mut writer: Vec<u8> = Vec::new();
            assert!(
                yes_no_accepted("go?", &mut reader, &mut writer).unwrap(),
                "input {:?} should be accepted",
                input
            );
        }
    }

    #[test]
    fn yes_no_accepted_rejects_empty_and_other() {
        for input in ["\n", "n\n", "no\n", "maybe\n"] {
            let mut reader = input.as_bytes();
            let mut writer: Vec<u8> = Vec::new();
            assert!(
                !yes_no_accepted("go?", &mut reader, &mut writer).unwrap(),
                "input {:?} should be rejected",
                input
            );
        }
    }

    #[test]
    fn render_is_initialized_detects_existing_config() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.json");
        fs::write(&config, b"{}").unwrap();

        assert!(render_is_initialized(Some(config.as_os_str()), None));
    }

    #[test]
    fn render_is_initialized_reports_missing_config() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("does-not-exist.json");

        assert!(!render_is_initialized(Some(missing.as_os_str()), None));
    }

    #[test]
    fn render_is_initialized_returns_false_without_home_or_override() {
        assert!(!render_is_initialized(None, None));
    }

    #[test]
    fn list_install_dirs_prefers_home_local_bin() {
        let dirs = list_install_dirs(Some(OsStr::new("/home/mag")));
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("/home/mag/.local/bin"),
                PathBuf::from("/usr/local/bin"),
            ]
        );
    }

    #[test]
    fn list_install_dirs_without_home_only_system_dir() {
        let dirs = list_install_dirs(None);
        assert_eq!(dirs, vec![PathBuf::from("/usr/local/bin")]);
    }

    #[test]
    fn prepend_to_path_puts_new_dirs_first_and_preserves_existing() {
        let current = env::join_paths([PathBuf::from("/usr/bin"), PathBuf::from("/bin")]).unwrap();
        let new = prepend_to_path(
            &[
                PathBuf::from("/home/mag/.local/bin"),
                PathBuf::from("/usr/local/bin"),
            ],
            &current,
        )
        .unwrap();

        let entries: Vec<PathBuf> = env::split_paths(&new).collect();
        assert_eq!(
            entries,
            vec![
                PathBuf::from("/home/mag/.local/bin"),
                PathBuf::from("/usr/local/bin"),
                PathBuf::from("/usr/bin"),
                PathBuf::from("/bin"),
            ]
        );
    }

    #[test]
    fn prepend_to_path_handles_empty_current() {
        let new = prepend_to_path(&[PathBuf::from("/a")], OsStr::new("")).unwrap();
        let entries: Vec<PathBuf> = env::split_paths(&new).collect();
        assert_eq!(entries.first(), Some(&PathBuf::from("/a")));
    }

    #[test]
    fn dir_in_path_detects_membership() {
        let joined = env::join_paths([PathBuf::from("/a"), PathBuf::from("/b/c")]).unwrap();
        assert!(dir_in_path(Path::new("/a"), &joined));
        assert!(dir_in_path(Path::new("/b/c"), &joined));
        assert!(!dir_in_path(Path::new("/nope"), &joined));
    }

    #[test]
    fn find_render_binary_dir_returns_first_matching() {
        let empty = TempDir::new().unwrap();
        let has_render = TempDir::new().unwrap();
        fs::write(has_render.path().join("render"), b"").unwrap();

        let dirs = vec![empty.path().to_path_buf(), has_render.path().to_path_buf()];
        assert_eq!(
            find_render_binary_dir(&dirs),
            Some(has_render.path().to_path_buf())
        );
    }

    #[test]
    fn find_render_binary_dir_returns_none_when_absent() {
        let dir = TempDir::new().unwrap();
        let dirs = vec![dir.path().to_path_buf()];
        assert_eq!(find_render_binary_dir(&dirs), None);
    }
}
