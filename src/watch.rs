use crate::build::Builder;
use crate::build::ChangeType;
use gitignored::Gitignore;
use notify::event::CreateKind;
use notify::event::DataChange;
use notify::event::ModifyKind;
use notify::Event;
use notify::EventKind;
use notify::RecursiveMode;
use notify::Watcher;
use std::fs::read_to_string;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::path::StripPrefixError;

#[derive(Debug, Clone)]
pub struct Config {
    pub current_dir: PathBuf,
    pub gitignore: Option<String>,
    pub builder: Builder,
}

impl Config {
    pub fn new(current_dir: &Path, builder: Builder) -> Self {
        Self {
            current_dir: current_dir.to_path_buf(),
            gitignore: read_to_string(".gitignore").ok(),
            builder,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Notify(notify::Error),
    IgnoredEvent(Event),
    EventFilePath(Event),
    RelativePath(StripPrefixError),
    IgnoredFileType(PathBuf),
}

pub fn watch(config: Config) {
    match _watch(config) {
        Ok(()) => {}
        Err(err) => {
            handle_error(err);
        }
    }
}

pub fn _watch(mut config: Config) -> Result<(), Error> {
    let mut watcher = notify::recommended_watcher(move |event_result| {
        match on_event(&mut config, event_result) {
            Ok(()) => {}
            Err(err) => handle_error(err),
        }
    })
    .map_err(|err| Error::Notify(err))?;

    watcher
        .watch(Path::new("."), RecursiveMode::Recursive)
        .map_err(|err| Error::Notify(err))?;

    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
    }
}

fn on_event(config: &mut Config, event_result: Result<Event, notify::Error>) -> Result<(), Error> {
    let event = event_result.map_err(|err| Error::Notify(err))?;
    let file_path = filepath_from_event(&event)?;
    let rel_path = file_path
        .strip_prefix(&config.current_dir)
        .map_err(|err| Error::RelativePath(err))?;

    let change_type = classify_file(&config, rel_path)?;
    config.builder.run(change_type);

    Ok(())
}

fn handle_error(err: Error) {
    match err {
        Error::Notify(err) => {
            eprintln!("Watcher error: {:?}", err);
        }

        Error::IgnoredEvent(_) => (),

        Error::EventFilePath(_) => {
            eprintln!("Failed to get path from event: {:?}", err);
        }

        Error::RelativePath(err) => {
            eprintln!("Failed to get relative path: {:?}", err);
        }

        Error::IgnoredFileType(_) => (),
    }
}

fn classify_file(config: &Config, path: &Path) -> Result<ChangeType, Error> {
    let extension = path.extension().unwrap_or_default();

    if is_ignored(config, path) {
        Err(Error::IgnoredFileType(path.to_path_buf()))
    } else if extension == "rs" {
        Ok(ChangeType::Rust)
    } else if extension == "ts" {
        Ok(ChangeType::TypeScript)
    } else {
        Err(Error::IgnoredFileType(path.to_path_buf()))
    }
}

fn is_ignored(config: &Config, path: &Path) -> bool {
    match &config.gitignore {
        Some(gitignore) => {
            let mut gi = Gitignore::new(&config.current_dir, false, false);
            let gitignore_lines: Vec<&str> = gitignore.lines().collect();
            gi.ignores(&gitignore_lines, gi.root.join(path))
        }

        None => false,
    }
}

fn filepath_from_event(event: &Event) -> Result<PathBuf, Error> {
    match &event.kind {
        EventKind::Create(create_kind) => {
            // Prevent rustfmt
            match create_kind {
                CreateKind::File => {
                    let path = event
                        .paths
                        .first()
                        .ok_or(Error::EventFilePath(event.clone()))?;

                    Ok(path.clone())
                }

                _ => Err(Error::IgnoredEvent(event.clone())),
            }
        }

        EventKind::Modify(modify_kind) => {
            // Prevent rustfmt
            match modify_kind {
                ModifyKind::Data(data_change) => {
                    // Prevent rustfmt
                    match data_change {
                        DataChange::Content => {
                            let path = event
                                .paths
                                .first()
                                .ok_or(Error::EventFilePath(event.clone()))?;

                            Ok(path.clone())
                        }

                        _ => Err(Error::IgnoredEvent(event.clone())),
                    }
                }

                ModifyKind::Name(_) => {
                    let path = event
                        .paths
                        .first()
                        .ok_or(Error::EventFilePath(event.clone()))?;

                    Ok(path.clone())
                }

                _ => Err(Error::IgnoredEvent(event.clone())),
            }
        }

        EventKind::Remove(_) => {
            let path = event
                .paths
                .first()
                .ok_or(Error::EventFilePath(event.clone()))?;

            Ok(path.clone())
        }

        _ => {
            // Prevent rustfmt
            Err(Error::IgnoredEvent(event.clone()))
        }
    }
}
