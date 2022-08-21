use std::io;
use std::path::PathBuf;
use std::process;
use std::process::Command;
use std::string;

#[derive(Debug)]
pub enum Error {
    FailedToExecute(io::Error),
    FailedToReadStdout(string::FromUtf8Error),
    FailedToReadStderr(string::FromUtf8Error),
    ExitFailure(String, Option<i32>),
}

pub struct Config {
    pub work_dir: PathBuf,
    pub cmd: String,
    pub args: Vec<String>,
}

pub fn to_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

pub fn run(config: &Config) -> Result<Output, Error> {
    Command::new(&config.cmd)
        .current_dir(&config.work_dir)
        .args(&config.args)
        .output()
        .map(|output| Output(output))
        .map_err(Error::FailedToExecute)
}

#[derive(Debug)]
pub struct Output(process::Output);

impl Output {
    pub fn into_stdout(self) -> Result<String, Error> {
        if self.0.status.success() {
            String::from_utf8(self.0.stdout).map_err(Error::FailedToReadStdout)
        } else {
            let stderr = String::from_utf8(self.0.stderr).map_err(Error::FailedToReadStderr)?;

            Err(Error::ExitFailure(stderr, self.0.status.code()))
        }
    }
}
