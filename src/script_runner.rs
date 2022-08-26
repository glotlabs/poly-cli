use crate::build::Env;
use crate::build::Runner;
use crate::exec;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    Exec(exec::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Error::Exec(err) => write!(f, "Script failed: {}", err),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScriptRunner {
    script_path: PathBuf,
    env: Env,
}

impl ScriptRunner {
    pub fn new(script_path: PathBuf, env: &Env) -> Self {
        Self {
            script_path,
            env: env.clone(),
        }
    }
}

impl Runner<Error> for ScriptRunner {
    fn run(&self) -> Result<(), Error> {
        exec::run(&exec::Config {
            work_dir: ".".into(),
            cmd: self.script_path.to_string_lossy().into(),
            args: vec![self.env.to_string()],
        })
        .map_err(Error::Exec)?;

        Ok(())
    }
}
