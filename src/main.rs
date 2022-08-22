mod build;
mod exec;
mod project_info;
mod rust_builder;
mod script_runner;
mod typescript_builder;
mod watch;

use crate::script_runner::ScriptRunner;
use build::Env;
use clap::{Parser, Subcommand};
use project_info::ProjectInfo;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[clap(name = "polyester")]
#[clap(about = "CLI helper for working with polyester projects", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Watch for changes and build
    #[clap(arg_required_else_help = true)]
    Watch {
        /// Post build script to run after build
        script: String,
    },
}

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Watch { script } => {
            let current_dir = get_current_dir();
            let script_path = current_dir.join(script);
            let project_info = ProjectInfo::from_dir(&current_dir).unwrap();
            let env = Env::Dev;

            print_project_info(&project_info);

            let builder = build::Builder::new(build::Config {
                rust_builder: rust_builder::RustBuilder::new(
                    rust_builder::Config::from_project_info(&env, &project_info),
                ),
                typescript_builder: typescript_builder::TypeScriptBuilder::new(
                    typescript_builder::Config::from_project_info(&env, &project_info),
                ),
                post_build_runner: script_path
                    .exists()
                    .then_some(ScriptRunner::new(script_path)),
            });

            let watcher_config = watch::Config::new(&current_dir, builder);

            println!("Watching for changes...");
            watch::watch(watcher_config);
        }
    }
}

fn get_current_dir() -> PathBuf {
    std::env::current_dir().unwrap()
}

fn print_project_info(info: &ProjectInfo) {
    println!("[Project name] {}", info.project_name);
    println!("[Dist dir] {}", info.dist_path.display());
    println!("[Web project dir] {}", info.web_project_path.display());
    println!("[Wasm project dir] {}", info.wasm_project_path.display());
    println!("");
}
