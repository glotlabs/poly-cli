mod backlog_builder;
mod build;
mod exec;
mod project;
mod project_info;
mod rust_builder;
mod script_runner;
mod typescript_builder;
mod watch;

use crate::backlog_builder::BacklogBuilder;
use crate::build::Runner;
use crate::project::Project;
use crate::rust_builder::RustBuilder;
use crate::script_runner::ScriptRunner;
use crate::typescript_builder::TypeScriptBuilder;
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
    /// Create a new project
    #[clap(arg_required_else_help = true)]
    New {
        /// Post build script to run after build
        name: String,
    },

    /// Build the project
    #[clap(arg_required_else_help = false)]
    Build {
        /// Release build
        #[clap(long)]
        release: bool,

        /// Post build script to run after build
        #[clap(long)]
        script: Option<String>,
    },

    /// Watch for changes and build
    #[clap(arg_required_else_help = true)]
    Watch {
        /// Post build script to run after build
        #[clap(long)]
        script: Option<String>,
    },
}

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::New { name } => {
            let current_dir = get_current_dir();
            let project = Project::new(project::Config {
                current_dir,
                name: name.clone(),
                template: project::Template::CounterTailwind,
            });

            let res = project.create();
            println!("{:?}", res);
        }

        Commands::Build { script, release } => {
            let env = if release { Env::Release } else { Env::Dev };
            let current_dir = get_current_dir();
            let project_info = ProjectInfo::from_dir(&current_dir).unwrap();

            print_project_info(&project_info);

            let rust_builder =
                RustBuilder::new(rust_builder::Config::from_project_info(&env, &project_info));

            let typescript_builder = TypeScriptBuilder::new(
                typescript_builder::Config::from_project_info(&env, &project_info),
            );

            rust_builder.run().expect("Rust build failed");
            typescript_builder.run().expect("TypeScript build failed");

            if let Some(script_name) = script {
                let script_path = current_dir.join(script_name);
                let script_runner = ScriptRunner::new(script_path);
                script_runner.run().expect("Post build runner failed");
            }
        }

        Commands::Watch { script } => {
            let env = Env::Dev;
            let current_dir = get_current_dir();
            let project_info = ProjectInfo::from_dir(&current_dir).unwrap();

            print_project_info(&project_info);

            let post_build_runner = if let Some(script_name) = script {
                let script_path = current_dir.join(script_name);
                if script_path.exists() {
                    Some(ScriptRunner::new(script_path))
                } else {
                    eprintln!("Could not find script: {}", script_path.display());
                    None
                }
            } else {
                None
            };

            let builder = BacklogBuilder::new(backlog_builder::Config {
                rust_builder: rust_builder::RustBuilder::new(
                    rust_builder::Config::from_project_info(&env, &project_info),
                ),
                typescript_builder: typescript_builder::TypeScriptBuilder::new(
                    typescript_builder::Config::from_project_info(&env, &project_info),
                ),
                post_build_runner,
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
