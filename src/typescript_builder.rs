use crate::build::CodeBuilder;
use crate::build::Env;
use crate::exec;
use crate::ProjectInfo;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub env: Env,
    pub web_project_path: PathBuf,
}

impl Config {
    pub fn from_project_info(env: &Env, project_info: &ProjectInfo) -> Self {
        Self {
            env: env.clone(),
            web_project_path: project_info.web_project_path.clone(),
        }
    }
}

pub enum Error {
    NpmBuildDev(exec::Error),
}

#[derive(Debug, Clone)]
pub struct TypeScriptBuilder {
    config: Config,
}

impl TypeScriptBuilder {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    fn build_release(&self) -> Result<(), Error> {
        Ok(())
    }

    fn build_dev(&self) -> Result<(), Error> {
        exec::run(&exec::Config {
            work_dir: self.config.web_project_path.clone(),
            cmd: "npm".into(),
            args: exec::to_args(&["run", "build-dev"]),
        })
        .map_err(Error::NpmBuildDev)?;

        Ok(())
    }
}

impl CodeBuilder<Error> for TypeScriptBuilder {
    fn build(&self) -> Result<(), Error> {
        match &self.config.env {
            Env::Dev => self.build_dev(),
            Env::Release => self.build_release(),
        }
    }
}
