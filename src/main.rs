mod asset_hasher;
mod backlog_builder;
mod build;
mod cleaner;
mod exec;
mod project;
mod project_info;
mod rust_builder;
mod script_runner;
mod serve;
mod util;
mod watch;
mod web_builder;

use crate::asset_hasher::AssetHasher;
use crate::backlog_builder::BacklogBuilder;
use crate::build::Runner;
use crate::cleaner::Cleaner;
use crate::project::Project;
use crate::rust_builder::RustBuilder;
use crate::script_runner::ScriptRunner;
use crate::web_builder::WebBuilder;
use build::Env;
use clap::{Parser, Subcommand};
use project_info::ProjectInfo;
use std::{path::PathBuf, process};

#[derive(Debug, Parser)]
#[clap(name = "poly")]
#[clap(about = "CLI helper for working with poly projects", long_about = None)]
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

    Add {
        #[clap(subcommand)]
        command: AddCommand,
    },

    /// Build the project
    #[clap(arg_required_else_help = false)]
    Build {
        /// Release build
        #[clap(long)]
        release: bool,

        /// Add filehash to filename of assets
        #[clap(long)]
        hash_assets: bool,

        /// Post build script to run after build
        #[clap(long)]
        script: Option<String>,
    },

    /// Watch for changes and build
    #[clap(arg_required_else_help = false)]
    Watch {
        /// Post build script to run after build
        #[clap(long)]
        script: Option<String>,
    },

    Serve {
        /// Path to serve static files from
        #[clap(long)]
        static_: Option<PathBuf>,

        /// Path to read routes from
        #[clap(long)]
        routes: Option<PathBuf>,

        /// Additional response headers
        #[clap(long)]
        header: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum AddCommand {
    /// Create a new project
    #[clap(arg_required_else_help = true)]
    Page {
        /// Page name
        name: String,
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

        Commands::Add { command } => {
            // fmt
            match command {
                AddCommand::Page { name } => {
                    let current_dir = get_current_dir();
                    let project_info = ProjectInfo::from_dir(&current_dir).unwrap();
                    let project = Project::new(project::Config {
                        current_dir: current_dir.clone(),
                        name: project_info.project_name.clone(),
                        template: project::Template::CounterTailwind,
                    });
                    let res = project.add_page(&project_info, &name);
                    println!("{:?}", res);
                }
            }
        }

        Commands::Build {
            script,
            release,
            hash_assets,
        } => {
            let env = if release { Env::Release } else { Env::Dev };
            let current_dir = get_current_dir();
            let project_info = ProjectInfo::from_dir(&current_dir).unwrap();

            print_project_info(&project_info);

            let cleaner = Cleaner::new(cleaner::Config::from_project_info(&project_info));

            let rust_builder =
                RustBuilder::new(rust_builder::Config::from_project_info(&env, &project_info));

            let web_builder =
                WebBuilder::new(web_builder::Config::from_project_info(&env, &project_info));

            cleaner.run().expect("Cleaner failed");

            if let Err(err) = rust_builder.run() {
                eprintln!("Rust build failed: {}", err);
                process::exit(1);
            }

            if let Err(err) = web_builder.run() {
                eprintln!("Web build failed: {}", err);
                process::exit(1);
            }

            if let Some(script_name) = &script {
                let script_path = current_dir.join(script_name);
                let script_runner = ScriptRunner::new(script_path, &env);
                script_runner
                    .run(script_runner::Event::BeforeAssetHash)
                    .expect("Post build runner failed");
            }

            if hash_assets {
                let asset_hasher =
                    AssetHasher::new(asset_hasher::Config::from_project_info(&project_info));

                let assets = asset_hasher.collect_hashed_dist_assets().unwrap();
                asset_hasher.update_uris_in_files(&assets).unwrap();

                rust_builder.run().expect("Rust build failed");
                web_builder.run().expect("Web build failed");

                asset_hasher.rename_assets(&assets).unwrap();

                if let Some(script_name) = &script {
                    let script_path = current_dir.join(script_name);
                    let script_runner = ScriptRunner::new(script_path, &env);
                    script_runner
                        .run(script_runner::Event::AfterAssetHash)
                        .expect("Post build runner failed");
                }
            }
        }

        Commands::Watch { script } => {
            let env = Env::Dev;
            let current_dir = get_current_dir();
            let project_info = ProjectInfo::from_dir(&current_dir).unwrap();

            print_project_info(&project_info);

            let cleaner = Cleaner::new(cleaner::Config::from_project_info(&project_info));

            let rust_builder = rust_builder::RustBuilder::new(
                rust_builder::Config::from_project_info(&env, &project_info),
            );

            let web_builder = web_builder::WebBuilder::new(web_builder::Config::from_project_info(
                &env,
                &project_info,
            ));

            let post_build_runner = if let Some(script_name) = script {
                let script_path = current_dir.join(script_name);
                if script_path.exists() {
                    Some(ScriptRunner::new(script_path, &env))
                } else {
                    eprintln!("Could not find script: {}", script_path.display());
                    None
                }
            } else {
                None
            };

            // Do initial build
            cleaner.run().expect("Cleaner failed");

            if let Err(err) = rust_builder.run() {
                eprintln!("Rust build failed: {}", err);
                process::exit(1);
            }

            if let Err(err) = web_builder.run() {
                eprintln!("Web build failed: {}", err);
                process::exit(1);
            }

            post_build_runner.as_ref().map(|runner| {
                runner
                    .run(script_runner::Event::BeforeAssetHash)
                    .expect("Post build runner failed")
            });

            let builder = BacklogBuilder::new(backlog_builder::Config {
                rust_builder,
                web_builder,
                post_build_runner,
            });

            println!("Watching for changes...");
            let watcher_config = watch::Config::new(&current_dir, builder);
            watch::watch(watcher_config);
        }

        Commands::Serve {
            static_,
            routes,
            header,
        } => {
            let default_path = get_current_dir().join("dist");
            let static_base_path = static_.unwrap_or(default_path);
            let parsed_routes = routes
                .map(|path| serve::read_routes(&path))
                .unwrap_or_default();

            let config = serve::Config {
                static_base_path,
                routes: parsed_routes,
                response_headers: header,
            };

            if let Err(err) = serve::start(&config) {
                eprintln!("Error: {:?}", err);
            }
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
    println!("[Core project dir] {}", info.core_project_path.display());
    println!("[Wasm project dir] {}", info.wasm_project_path.display());
    println!(
        "[Cloudflare project dir] {}",
        info.cloudflare_project_path.display()
    );
    println!("");
}
