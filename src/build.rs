use crate::rust_builder;
use crate::rust_builder::RustBuilder;
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

pub enum Error {
    BacklogLock(String),
    RustBuild(rust_builder::Error),
    TypescriptBuild(typescript_builder::Error),
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
    //post_build_script: Option<PathBuf>,
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
            run_script(build_type, &config);

            state
                .is_running
                .store(false, std::sync::atomic::Ordering::Relaxed);

            build(config, state);
        });

        Ok(())
    } else {
        Ok(())
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

fn run_script(build_type: BuildType, config: &Config) -> Result<(), Error> {
    println!("Running build: {:?}", build_type);

    match build_type {
        BuildType::All => {
            config.rust_builder.build().map_err(Error::RustBuild)?;
            config
                .typescript_builder
                .build()
                .map_err(Error::TypescriptBuild)?;
        }

        BuildType::OnlyTypeScript => {
            config
                .typescript_builder
                .build()
                .map_err(Error::TypescriptBuild)?;
        }
    }

    //let output =
    //    exec::run(&script, &[]).map_err(|err| format!("Failed to run script: {:?}", err))?;

    //println!("Output: {:?}", output.into_stdout());
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
