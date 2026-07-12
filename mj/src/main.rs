use std::env;
use std::process::ExitCode;

mod args;
mod psql;
mod render;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let Some(cmd) = args.get(1) else {
        print_usage();
        return ExitCode::from(1);
    };

    match cmd.as_str() {
        "render" => render::cmd_render(&args[2..]),
        "psql" => psql::cmd_psql(&args[2..]),
        "-h" | "--help" | "help" => {
            print_usage();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("mj: unknown command '{other}'");
            print_usage();
            ExitCode::from(1)
        }
    }
}

fn print_usage() {
    println!("Usage: mj <command>");
    println!();
    println!("Commands:");
    println!("  render    Manage the Render CLI (see `mj render` for subcommands)");
    println!("  psql      Postgres helpers (see `mj psql` for subcommands)");
}
