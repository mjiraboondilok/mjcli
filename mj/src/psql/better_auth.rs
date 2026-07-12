use crate::args::{parse_single_value_flag_or_usage, parse_value_flags_or_usage};
use rand::Rng;
use rand::distr::{Alphanumeric, SampleString};
use std::env;
use std::io;
use std::process::{Command, ExitCode};

const CORE_TABLES: [&str; 4] = ["user", "session", "account", "verification"];

const CREATE_TABLES_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS "user" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "name" TEXT NOT NULL,
    "email" TEXT NOT NULL UNIQUE,
    "emailVerified" BOOLEAN NOT NULL,
    "image" TEXT,
    "createdAt" TIMESTAMP NOT NULL,
    "updatedAt" TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS "session" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "expiresAt" TIMESTAMP NOT NULL,
    "token" TEXT NOT NULL UNIQUE,
    "createdAt" TIMESTAMP NOT NULL,
    "updatedAt" TIMESTAMP NOT NULL,
    "ipAddress" TEXT,
    "userAgent" TEXT,
    "userId" TEXT NOT NULL REFERENCES "user" ("id")
);

CREATE TABLE IF NOT EXISTS "account" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "accountId" TEXT NOT NULL,
    "providerId" TEXT NOT NULL,
    "userId" TEXT NOT NULL REFERENCES "user" ("id"),
    "accessToken" TEXT,
    "refreshToken" TEXT,
    "idToken" TEXT,
    "accessTokenExpiresAt" TIMESTAMP,
    "refreshTokenExpiresAt" TIMESTAMP,
    "scope" TEXT,
    "password" TEXT,
    "createdAt" TIMESTAMP NOT NULL,
    "updatedAt" TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS "verification" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "identifier" TEXT NOT NULL,
    "value" TEXT NOT NULL,
    "expiresAt" TIMESTAMP NOT NULL,
    "createdAt" TIMESTAMP,
    "updatedAt" TIMESTAMP
);
"#;

pub(super) fn cmd_psql_better_auth(args: &[String]) -> ExitCode {
    let Some(sub) = args.first() else {
        print_usage();
        return ExitCode::from(1);
    };

    match sub.as_str() {
        "check" => run_with_connection(&args[1..], sub, check),
        "init" => run_with_connection(&args[1..], sub, init),
        "insert" => cmd_insert(&args[1..], sub),
        "-h" | "--help" | "help" => {
            print_usage();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("mj psql better-auth: unknown subcommand '{other}'");
            print_usage();
            ExitCode::from(1)
        }
    }
}

fn run_with_connection(
    args: &[String],
    subcommand: &str,
    action: fn(Option<&str>, &str) -> ExitCode,
) -> ExitCode {
    match parse_connection(args, subcommand) {
        Ok(connection) => action(connection.as_deref(), subcommand),
        Err(code) => code,
    }
}

fn parse_connection(args: &[String], subcommand: &str) -> Result<Option<String>, ExitCode> {
    parse_single_value_flag_or_usage(
        args,
        "--connection",
        &format!("Usage: mj psql better-auth {subcommand} [--connection <CONNECTION>]"),
    )
}

fn print_usage() {
    println!("Usage: mj psql better-auth <subcommand>");
    println!();
    println!("Subcommands:");
    println!("  check [--connection <CONNECTION>]    Check that psql connects and that");
    println!("                                          the better-auth tables exist");
    println!("  init [--connection <CONNECTION>]     Create the core better-auth tables");
    println!("                                          (user, session, account, verification)");
    println!("  insert --email <EMAIL> [--connection <CONNECTION>]");
    println!("                                        Create a user with a credential account");
    println!("                                          and print its generated password");
}

fn check(connection: Option<&str>, subcommand: &str) -> ExitCode {
    let output = match run_psql(&table_lookup_query(), connection) {
        Ok(output) => output,
        Err(e) if e.connection_failed => {
            report_failure_with_hint(
                "Could not connect to Postgres with psql.",
                subcommand,
                &e.message,
            );
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("Connected, but could not query the database schema.");
            eprintln!("{}", e.message);
            return ExitCode::FAILURE;
        }
    };
    println!("Connected to the Postgres server.");

    let (found, missing) = partition_tables(&output);
    if missing.is_empty() {
        println!(
            "All better-auth tables are present ({}).",
            core_table_list()
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("Missing better-auth tables: {}.", missing.join(", "));
        if found.is_empty() {
            eprintln!("None of the better-auth tables exist in this database.");
        } else {
            eprintln!("Found: {}.", found.join(", "));
        }
        eprintln!("Run your better-auth migrations to create them.");
        ExitCode::FAILURE
    }
}

fn init(connection: Option<&str>, subcommand: &str) -> ExitCode {
    if let Err(e) = run_psql(CREATE_TABLES_SQL, connection) {
        report_failure_with_hint(
            "Could not create the better-auth tables.",
            subcommand,
            &e.message,
        );
        return ExitCode::FAILURE;
    }

    println!("Created the better-auth tables ({}).", core_table_list());
    ExitCode::SUCCESS
}

const INSERT_USAGE: &str =
    "Usage: mj psql better-auth insert --email <EMAIL> [--connection <CONNECTION>]";

fn cmd_insert(args: &[String], subcommand: &str) -> ExitCode {
    let (email, connection) = match parse_insert_args(args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    insert(&email, connection.as_deref(), subcommand)
}

fn parse_insert_args(args: &[String]) -> Result<(String, Option<String>), ExitCode> {
    let [email, connection] =
        parse_value_flags_or_usage(args, &["--email", "--connection"], INSERT_USAGE)?;
    let email = email.ok_or_else(|| {
        eprintln!("error: --email is required");
        eprintln!("{INSERT_USAGE}");
        ExitCode::FAILURE
    })?;
    Ok((email, connection))
}

fn insert(email: &str, connection: Option<&str>, subcommand: &str) -> ExitCode {
    let email = email.to_lowercase();
    let mut rng = rand::rng();
    let password = generate_random_string(&mut rng, 24);
    let password_hash = hash_password(&password, &mut rng);
    let user_id = generate_random_string(&mut rng, 32);
    let account_id = generate_random_string(&mut rng, 32);

    let sql = insert_sql(&user_id, &account_id, &email, &password_hash);
    if let Err(e) = run_psql(&sql, connection) {
        report_failure_with_hint("Could not create the user.", subcommand, &e.message);
        return ExitCode::FAILURE;
    }

    println!("Created user {email} (id: {user_id}).");
    println!("Password: {password}");
    ExitCode::SUCCESS
}

fn insert_sql(user_id: &str, account_id: &str, email: &str, password_hash: &str) -> String {
    format!(
        r#"
INSERT INTO "user" ("id", "name", "email", "emailVerified", "createdAt", "updatedAt")
VALUES ({user_id}, {email}, {email}, FALSE, now(), now());

INSERT INTO "account" ("id", "accountId", "providerId", "userId", "password", "createdAt", "updatedAt")
VALUES ({account_id}, {user_id}, 'credential', {user_id}, {password_hash}, now(), now());
"#,
        user_id = sql_quote(user_id),
        email = sql_quote(email),
        account_id = sql_quote(account_id),
        password_hash = sql_quote(password_hash),
    )
}

fn sql_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn generate_random_string(rng: &mut impl Rng, len: usize) -> String {
    Alphanumeric.sample_string(rng, len)
}

// Matches better-auth's default scrypt config (N = 2^14, r = 16, p = 1, 64-byte
// derived key, stored as `salt:hex(key)`), so passwords created here verify
// against better-auth's own login flow.
const SCRYPT_LOG_N: u8 = 14;
const SCRYPT_R: u32 = 16;
const SCRYPT_P: u32 = 1;
const SCRYPT_DK_LEN: usize = 64;

fn hash_password(password: &str, rng: &mut impl Rng) -> String {
    let mut salt_bytes = [0u8; 16];
    rng.fill_bytes(&mut salt_bytes);
    let salt_hex = hex_encode(&salt_bytes);

    let params = scrypt::Params::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P)
        .expect("static scrypt params are valid");
    let mut key = [0u8; SCRYPT_DK_LEN];
    scrypt::scrypt(password.as_bytes(), salt_hex.as_bytes(), &params, &mut key)
        .expect("fixed-size output buffer is valid");

    format!("{salt_hex}:{}", hex_encode(&key))
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{b:02x}").expect("writing to a String never fails");
    }
    s
}

fn core_table_list() -> String {
    CORE_TABLES.join(", ")
}

fn report_failure_with_hint(message: &str, subcommand: &str, e: &str) {
    eprintln!("{message}");
    eprintln!("{e}");
    eprintln!();
    eprintln!("Check your psql configuration (PGHOST, PGPORT, PGUSER, PGDATABASE, PGPASSWORD,");
    eprintln!("~/.pgpass), or pass one: mj psql better-auth {subcommand} --connection <URL>");
}

fn table_lookup_query() -> String {
    let names = CORE_TABLES.map(|t| format!("'{t}'")).join(", ");
    format!("SELECT table_name FROM information_schema.tables WHERE table_name IN ({names});")
}

fn partition_tables(psql_output: &str) -> (Vec<&'static str>, Vec<&'static str>) {
    CORE_TABLES
        .iter()
        .copied()
        .partition(|t| psql_output.lines().any(|line| line.trim() == *t))
}

struct PsqlError {
    message: String,
    /// psql exits with status 2 specifically when it could not connect to the
    /// server (as opposed to e.g. a query error), letting us tell the two
    /// apart without a second round-trip to the database.
    connection_failed: bool,
}

fn run_psql(sql: &str, connection: Option<&str>) -> Result<String, PsqlError> {
    let mut cmd = Command::new("psql");
    cmd.args(["--no-psqlrc", "--tuples-only", "--no-align", "--quiet"]);
    if env::var_os("PGCONNECT_TIMEOUT").is_none() {
        cmd.env("PGCONNECT_TIMEOUT", "10");
    }
    if let Some(conn) = connection {
        cmd.arg("--dbname").arg(conn);
    }
    cmd.arg("--command").arg(sql);

    let output = cmd.output().map_err(|e| PsqlError {
        connection_failed: true,
        message: if e.kind() == io::ErrorKind::NotFound {
            "The `psql` command was not found on your PATH.\n\
                Install the PostgreSQL client tools and try again."
                .to_owned()
        } else {
            format!("Failed to run psql: {e}")
        },
    })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(PsqlError {
            connection_failed: output.status.code() == Some(2),
            message: if stderr.is_empty() {
                match output.status.code() {
                    Some(code) => format!("psql exited with status {code}."),
                    None => "psql was terminated by a signal.".to_owned(),
                }
            } else {
                stderr
                    .lines()
                    .map(|l| format!("  {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_quote_escapes_single_quotes() {
        assert_eq!(sql_quote("o'brien@example.com"), "'o''brien@example.com'");
        assert_eq!(sql_quote("plain@example.com"), "'plain@example.com'");
    }

    #[test]
    fn insert_sql_reuses_user_id_as_account_id_and_uses_credential_provider() {
        let sql = insert_sql("user123", "acct456", "a@example.com", "salt:key");
        assert!(sql.contains(r#"INSERT INTO "user""#));
        assert!(sql.contains(r#"INSERT INTO "account""#));
        assert!(sql.contains("'credential'"));
        assert!(sql.contains("'user123'"));
        assert!(sql.contains("'acct456'"));
        assert!(sql.contains("'a@example.com'"));
        assert!(sql.contains("'salt:key'"));
    }

    #[test]
    fn generate_random_string_has_requested_length_and_is_alphanumeric() {
        let s = generate_random_string(&mut rand::rng(), 32);
        assert_eq!(s.len(), 32);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn hash_password_round_trips_with_a_from_scratch_scrypt_verify() {
        let password = "correct horse battery staple";
        let hash = hash_password(password, &mut rand::rng());

        let (salt_hex, key_hex) = hash.split_once(':').expect("salt:key format");
        assert_eq!(salt_hex.len(), 32);
        assert_eq!(key_hex.len(), 128);

        let params = scrypt::Params::new(14, 16, 1).unwrap();
        let mut key = [0u8; 64];
        scrypt::scrypt(password.as_bytes(), salt_hex.as_bytes(), &params, &mut key).unwrap();
        assert_eq!(hex_encode(&key), key_hex);

        let mut wrong_key = [0u8; 64];
        scrypt::scrypt(
            b"wrong password",
            salt_hex.as_bytes(),
            &params,
            &mut wrong_key,
        )
        .unwrap();
        assert_ne!(hex_encode(&wrong_key), key_hex);
    }

    #[test]
    fn parse_insert_args_requires_email() {
        let args: Vec<String> = vec![];
        assert!(parse_insert_args(&args).is_err());
    }

    #[test]
    fn parse_insert_args_parses_email_and_connection() {
        let args: Vec<String> = ["--email", "a@example.com", "--connection", "postgres://x"]
            .into_iter()
            .map(String::from)
            .collect();
        let (email, connection) = parse_insert_args(&args).unwrap();
        assert_eq!(email, "a@example.com");
        assert_eq!(connection.as_deref(), Some("postgres://x"));
    }

    #[test]
    fn all_tables_present_reports_nothing_missing() {
        let output = "user\nsession\naccount\nverification\n";
        let (found, missing) = partition_tables(output);
        assert!(missing.is_empty());
        assert_eq!(found, CORE_TABLES.to_vec());
    }

    #[test]
    fn missing_tables_are_reported_in_schema_order() {
        let output = "user\naccount\n";
        let (found, missing) = partition_tables(output);
        assert_eq!(missing, vec!["session", "verification"]);
        assert_eq!(found, vec!["user", "account"]);
    }

    #[test]
    fn empty_output_means_all_missing() {
        let (found, missing) = partition_tables("   \n\n");
        assert!(found.is_empty());
        assert_eq!(missing, vec!["user", "session", "account", "verification"]);
    }

    #[test]
    fn lookup_query_names_all_core_tables() {
        let query = table_lookup_query();
        for table in CORE_TABLES {
            assert!(query.contains(&format!("'{table}'")), "missing {table}");
        }
    }

    #[test]
    fn create_tables_sql_creates_all_core_tables_idempotently() {
        for table in CORE_TABLES {
            assert!(
                CREATE_TABLES_SQL.contains(&format!("CREATE TABLE IF NOT EXISTS \"{table}\"")),
                "missing CREATE TABLE IF NOT EXISTS for {table}"
            );
        }
    }

    #[test]
    fn create_tables_sql_declares_user_before_its_dependents() {
        let user_pos = CREATE_TABLES_SQL.find("\"user\" (").unwrap();
        for dependent in ["session", "account"] {
            let dependent_pos = CREATE_TABLES_SQL
                .find(&format!("CREATE TABLE IF NOT EXISTS \"{dependent}\""))
                .unwrap();
            assert!(
                user_pos < dependent_pos,
                "\"user\" table must be declared before {dependent}"
            );
        }
    }
}
