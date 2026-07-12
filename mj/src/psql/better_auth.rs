use crate::args::parse_single_value_flag_or_usage;
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
        println!("All better-auth tables are present ({}).", core_table_list());
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
