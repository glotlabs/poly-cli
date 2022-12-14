use crate::build::Env;
use crate::build::Runner;
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

    fn web_project_wasm_path(&self) -> PathBuf {
        self.web_project_path.join("wasm")
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

    fn build_dev(&self) -> Result<(), Error> {
        self.prepare_dirs()?;

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
                "--dev",
                "--target",
                "web",
                "--out-name",
                &self.config.project_name,
                "--out-dir",
                &self.config.web_project_wasm_path().to_string_lossy(),
            ]),
        })
        .map_err(Error::WasmPack)?;

        self.copy_wasm_to_dist()?;

        Ok(())
    }

    fn build_release(&self) -> Result<(), Error> {
        self.prepare_dirs()?;

        exec::run(&exec::Config {
            work_dir: ".".into(),
            cmd: "cargo".into(),
            args: exec::to_args(&["build", "--release", "--color", "always"]),
        })
        .map_err(Error::CargoBuild)?;

        exec::run(&exec::Config {
            work_dir: self.config.wasm_project_path.clone(),
            cmd: "wasm-pack".into(),
            args: exec::to_args(&[
                "build",
                "--release",
                "--target",
                "web",
                "--out-name",
                &self.config.project_name,
                "--out-dir",
                &self.config.web_project_wasm_path().to_string_lossy(),
            ]),
        })
        .map_err(Error::WasmPack)?;

        self.copy_wasm_to_dist()?;

        Ok(())
    }

    fn prepare_dirs(&self) -> Result<(), Error> {
        fs::create_dir_all(&self.config.dist_path).map_err(Error::CreateDistDir)?;
        fs::create_dir_all(&self.config.web_project_wasm_path())
            .map_err(Error::CreateWebWasmDir)?;

        Ok(())
    }

    fn copy_wasm_to_dist(&self) -> Result<(), Error> {
        fs_extra::dir::copy(
            &self.config.web_project_wasm_path(),
            &self.config.dist_path,
            &fs_extra::dir::CopyOptions {
                overwrite: true,
                ..fs_extra::dir::CopyOptions::default()
            },
        )
        .map_err(Error::CopyWasmToDist)?;

        Ok(())
    }
}

impl Runner<Error> for RustBuilder {
    fn run(&self) -> Result<(), Error> {
        match &self.config.env {
            Env::Dev => self.build_dev(),
            Env::Release => self.build_release(),
        }
    }
}
