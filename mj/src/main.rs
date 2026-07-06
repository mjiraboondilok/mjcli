use std::env;
use std::process::ExitCode;

mod render;

fn main() -> ExitCode {
    let Some(cmd) = env::args().nth(1) else {
        print_usage();
        return ExitCode::from(1);
    };

    match cmd.as_str() {
        "render" => render::cmd_render(),
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
    println!("  render    Ensure the Render CLI is installed and initialized");
}
