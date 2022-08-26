use std::fmt;
use std::fmt::Display;

pub trait Runner<E> {
    fn run(&self) -> Result<(), E>;
}

#[derive(Debug, Clone)]
pub enum Env {
    Dev,
    Release,
}

impl Display for Env {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Env::Dev => write!(f, "dev"),
            Env::Release => write!(f, "release"),
        }
    }
}
