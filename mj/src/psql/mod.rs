use crate::psql::better_auth::cmd_psql_better_auth;
use std::process::ExitCode;

mod better_auth;

pub fn cmd_psql(args: &[String]) -> ExitCode {
    let Some(sub) = args.first() else {
        print_usage();
        return ExitCode::from(1);
    };

    match sub.as_str() {
        "better-auth" => cmd_psql_better_auth(&args[1..]),
        "-h" | "--help" | "help" => {
            print_usage();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("mj psql: unknown subcommand '{other}'");
            print_usage();
            ExitCode::from(1)
        }
    }
}

fn print_usage() {
    println!("Usage: mj psql <subcommand>");
    println!();
    println!("Subcommands:");
    println!(
        "  better-auth    Manage better-auth tables (see `mj psql better-auth` for subcommands)"
    );
}
