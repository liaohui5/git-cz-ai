use clap::Command;

use git_cz_ai::{ai, config, manually};

fn main() {
    if let Err(e) = run() {
        eprintln!("错误：{e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("git-cz")
        .about("Simple commitizen CLI tool written in Rust")
        .subcommand(config::create_init_config_cmd())
        .subcommand(ai::create_ai_cmd())
        .get_matches();

    match matches.subcommand() {
        Some(("ai", args)) => ai::handler(args),
        Some(("init-config", _)) => config::handler(),
        _ => manually::handler(),
    }
}
