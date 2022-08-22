use crate::build::CodeBuilder;
use crate::build::Env;
use crate::exec;
use crate::ProjectInfo;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub env: Env,
    pub project_name: String,
    pub dist_path: PathBuf,
    pub web_project_path: PathBuf,
    pub wasm_project_path: PathBuf,
}

impl Config {
    pub fn from_project_info(env: &Env, project_info: &ProjectInfo) -> Self {
        Self {
            env: env.clone(),
            project_name: project_info.project_name.clone(),
            dist_path: project_info.dist_path.clone(),
            web_project_path: project_info.web_project_path.clone(),
            wasm_project_path: project_info.wasm_project_path.clone(),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    CreateDistDir(io::Error),
    CreateWebWasmDir(io::Error),
    CargoBuild(exec::Error),
    WasmPack(exec::Error),
    CopyWasmToDist(fs_extra::error::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Error::CreateDistDir(err) => write!(f, "Failed to create the dist dir: {}", err),

            Error::CreateWebWasmDir(err) => {
                write!(f, "Failed to create the wasm dir in web project: {}", err)
            }

            Error::CargoBuild(err) => write!(f, "cargo build failed: {}", err),

            Error::WasmPack(err) => write!(f, "wasm-pack failed: {}", err),

            Error::CopyWasmToDist(err) => write!(f, "Failed to copy wasm dir to dist: {}", err),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RustBuilder {
    config: Config,
}

impl RustBuilder {
    pub fn new(config: Config) -> Self {
        Self { config: config }
    }

    fn build_release(&self) -> Result<(), Error> {
        Ok(())
    }

    fn build_dev(&self) -> Result<(), Error> {
        let _ = fs::remove_dir_all(&self.config.dist_path);
        fs::create_dir_all(&self.config.dist_path).map_err(Error::CreateDistDir)?;

        let web_project_wasm_path = self.config.web_project_path.join("wasm");
        let _ = fs::remove_dir_all(&web_project_wasm_path);
        fs::create_dir_all(&web_project_wasm_path).map_err(Error::CreateWebWasmDir)?;

        exec::run(&exec::Config {
            work_dir: ".".into(),
            cmd: "cargo".into(),
            args: exec::to_args(&["build", "--color", "always"]),
        })
        .map_err(Error::CargoBuild)?;

        exec::run(&exec::Config {
            work_dir: self.config.wasm_project_path.clone(),
            cmd: "wasm-pack".into(),
            args: exec::to_args(&[
                "build",
                "--target",
                "web",
                "--out-name",
                &self.config.project_name,
                "--out-dir",
                &web_project_wasm_path.to_string_lossy(),
            ]),
        })
        .map_err(Error::WasmPack)?;

        fs_extra::dir::copy(
            &web_project_wasm_path,
            &self.config.dist_path,
            &fs_extra::dir::CopyOptions::new(),
        )
        .map_err(Error::CopyWasmToDist)?;

        Ok(())
    }
}

impl CodeBuilder<Error> for RustBuilder {
    fn build(&self) -> Result<(), Error> {
        match &self.config.env {
            Env::Dev => self.build_dev(),
            Env::Release => self.build_release(),
        }
    }
}
