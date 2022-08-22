use crate::rust_builder;
use crate::rust_builder::RustBuilder;
use crate::script_runner;
use crate::script_runner::ScriptRunner;
use crate::typescript_builder;
use crate::typescript_builder::TypeScriptBuilder;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ChangeType {
    Rust,
    TypeScript,
}

#[derive(Debug)]
pub enum Error {
    BacklogLock(String),
}

#[derive(Debug)]
pub enum BuildError {
    RustBuild(rust_builder::Error),
    TypescriptBuild(typescript_builder::Error),
    PostBuildRunner(script_runner::Error),
}

#[derive(Debug, Clone)]
pub struct Builder {
    config: Config,
    state: Arc<State>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub rust_builder: RustBuilder,
    pub typescript_builder: TypeScriptBuilder,
    pub post_build_runner: Option<ScriptRunner>,
}

impl Builder {
    pub fn new(config: Config) -> Self {
        Self {
            config: config,
            state: Arc::new(State::new()),
        }
    }

    pub fn run(&mut self, change: ChangeType) -> Result<(), Error> {
        self.state
            .backlog
            .lock()
            .map_err(|err| Error::BacklogLock(err.to_string()))?
            .insert(change);

        if self.is_running() {
            Ok(())
        } else {
            build(self.config.clone(), self.state.clone())
        }
    }

    fn is_running(&self) -> bool {
        self.state
            .is_running
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

fn build(config: Config, state: Arc<State>) -> Result<(), Error> {
    let backlog_length = state
        .backlog
        .lock()
        .map_err(|err| Error::BacklogLock(err.to_string()))?
        .len();

    if backlog_length > 0 {
        state
            .is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let changes: HashSet<ChangeType> = state
            .backlog
            .lock()
            .map_err(|err| Error::BacklogLock(err.to_string()))?
            .drain()
            .collect();

        let build_type = BuildType::from_changes(changes);

        std::thread::spawn(move || {
            if let Err(err) = run_script(build_type, &config) {
                handle_build_error(err);
            };

            state
                .is_running
                .store(false, std::sync::atomic::Ordering::Relaxed);

            if let Err(err) = build(config, state) {
                handle_error(err);
            }
        });

        Ok(())
    } else {
        Ok(())
    }
}
pub fn handle_error(err: Error) {
    match err {
        Error::BacklogLock(err) => {
            println!("Failed to get a lock on backlog: {}", err);
        }
    }
}

fn handle_build_error(err: BuildError) {
    match err {
        BuildError::RustBuild(err) => {
            // Prevent rustfmt
            println!("Rust build failed: {}", err);
        }

        BuildError::TypescriptBuild(err) => {
            // Prevent rustfmt
            println!("TypeScript build failed: {}", err);
        }

        BuildError::PostBuildRunner(err) => {
            // Prevent rustfmt
            println!("Post-build script failed: {}", err);
        }
    }
}

#[derive(Debug)]
pub struct State {
    is_running: AtomicBool,
    backlog: Mutex<HashSet<ChangeType>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            is_running: AtomicBool::new(false),
            backlog: Mutex::new(HashSet::new()),
        }
    }
}

fn run_script(build_type: BuildType, config: &Config) -> Result<(), BuildError> {
    println!("\nStarting build of {:?}", build_type);

    match build_type {
        BuildType::All => {
            config.rust_builder.build().map_err(BuildError::RustBuild)?;
            config
                .typescript_builder
                .build()
                .map_err(BuildError::TypescriptBuild)?;
        }

        BuildType::OnlyTypeScript => {
            config
                .typescript_builder
                .build()
                .map_err(BuildError::TypescriptBuild)?;
        }
    }

    if let Some(post_build_runner) = &config.post_build_runner {
        post_build_runner
            .build()
            .map_err(BuildError::PostBuildRunner)?;
    }

    println!("Completed build of {:?}", build_type);

    Ok(())
}

#[derive(Debug)]
enum BuildType {
    All,
    OnlyTypeScript,
}

impl BuildType {
    fn from_changes(changes: HashSet<ChangeType>) -> BuildType {
        let only_typescript = HashSet::from([ChangeType::TypeScript]);

        if changes == only_typescript {
            BuildType::OnlyTypeScript
        } else {
            BuildType::All
        }
    }
}

pub trait CodeBuilder<E> {
    fn build(&self) -> Result<(), E>;
}

#[derive(Debug, Clone)]
pub enum Env {
    Dev,
    Release,
}
