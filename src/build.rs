pub trait Runner<E> {
    fn run(&self) -> Result<(), E>;
}

#[derive(Debug, Clone)]
pub enum Env {
    Dev,
    Release,
}
