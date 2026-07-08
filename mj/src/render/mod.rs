use crate::render::exit::cmd_render_exit;
use crate::render::init::cmd_render_init;
use std::process::ExitCode;

mod exit;
mod init;
mod shared;

pub fn cmd_render(args: &[String]) -> ExitCode {
    let Some(sub) = args.first() else {
        print_usage();
        return ExitCode::from(1);
    };

    match sub.as_str() {
        "init" => cmd_render_init(&args[1..]),
        "exit" => cmd_render_exit(),
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
    println!("  init [--api-key <KEY>]    Save and validate a Render API key");
    println!("  exit                      Delete the saved Render API key");
}
