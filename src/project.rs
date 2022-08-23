use std::fs;
use std::io;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct Config {
    pub name: String,
    pub template: Template,
    pub current_dir: PathBuf,
}

pub struct Project {
    config: Config,
}

#[derive(Debug)]
pub enum Error {
    TempDir(io::Error),
    GetUrl(ureq::Error),
    ReadResponse(io::Error),
    ZipExtract(zip_extract::ZipExtractError),
    ReadFile(io::Error),
    WriteFile(io::Error),
    RenameFile(io::Error),
    RenameDir(io::Error),
    CopyToDestination(fs_extra::error::Error),
    RenameTemplateDir(io::Error),
}

impl Project {
    pub fn new(config: Config) -> Project {
        Project { config }
    }

    pub fn create(&self) -> Result<(), Error> {
        let template_info = self.config.template.info();
        let temp_dir = tempfile::tempdir().map_err(Error::TempDir)?;
        let temp_dir_path = temp_dir.path();
        let template_dir = temp_dir_path.join(&template_info.path);

        let bytes = self.download_file(&template_info)?;
        self.extract_zip(bytes, temp_dir_path)?;
        self.replace_placeholders(&template_info, &template_dir)?;
        self.copy_to_dest(&template_dir, &self.config.current_dir)?;

        Ok(())
    }

    fn copy_to_dest(&self, template_dir: &PathBuf, dest: &PathBuf) -> Result<(), Error> {
        let tmp_project_path = template_dir.with_file_name(&self.config.name);
        fs::rename(&template_dir, &tmp_project_path).map_err(Error::RenameTemplateDir)?;

        fs_extra::dir::copy(tmp_project_path, dest, &fs_extra::dir::CopyOptions::new())
            .map_err(Error::CopyToDestination)?;

        Ok(())
    }

    fn download_file(&self, template_info: &TemplateInfo) -> Result<Vec<u8>, Error> {
        let response = ureq::get(&template_info.url)
            .call()
            .map_err(Error::GetUrl)?;

        let mut buffer = Vec::new();

        response
            .into_reader()
            .read_to_end(&mut buffer)
            .map_err(Error::ReadResponse)?;

        Ok(buffer)
    }

    fn extract_zip(&self, bytes: Vec<u8>, base_path: &Path) -> Result<(), Error> {
        let mut cursor = Cursor::new(bytes);
        zip_extract::extract(&mut cursor, base_path, true).map_err(Error::ZipExtract)?;

        Ok(())
    }

    fn replace_placeholders(
        &self,
        template_info: &TemplateInfo,
        template_dir: &PathBuf,
    ) -> Result<(), Error> {
        let paths = self.collect_dir_entries(template_dir);

        paths
            .files
            .iter()
            .map(|path| self.replace_placeholder_in_file(template_info, path))
            .collect::<Result<(), Error>>()?;

        paths
            .dirs
            .iter()
            .map(|path| self.replace_placeholder_in_dir(template_info, path))
            .collect::<Result<(), Error>>()?;

        Ok(())
    }

    fn collect_dir_entries(&self, template_dir: &PathBuf) -> Paths {
        let entries = WalkDir::new(template_dir).into_iter().filter_map(|entry| {
            match entry {
                Ok(entry) => {
                    //fmt
                    Some(entry)
                }

                Err(err) => {
                    eprintln!("Warning: Can't access file: {}", err);
                    None
                }
            }
        });

        let mut files: Vec<PathBuf> = Vec::new();
        let mut dirs: Vec<PathBuf> = Vec::new();

        for entry in entries {
            let file_type = entry.file_type();

            if file_type.is_file() {
                files.push(entry.path().to_path_buf());
            } else if file_type.is_dir() {
                dirs.push(entry.path().to_path_buf());
            }
        }

        Paths { files, dirs }
    }

    fn replace_placeholder_in_file(
        &self,
        template_info: &TemplateInfo,
        file_path: &PathBuf,
    ) -> Result<(), Error> {
        let tmp_file_path = file_path.with_extension("tmp");
        let old_content = fs::read_to_string(file_path).map_err(Error::ReadFile)?;
        let new_content = old_content.replace(&template_info.placeholder, &self.config.name);

        println!(
            "Replacing placeholder: {} -> {} in {}",
            template_info.placeholder,
            self.config.name,
            file_path.display()
        );

        fs::write(&tmp_file_path, new_content).map_err(Error::WriteFile)?;
        fs::rename(&tmp_file_path, file_path).map_err(Error::RenameFile)?;

        Ok(())
    }

    fn replace_placeholder_in_dir(
        &self,
        template_info: &TemplateInfo,
        dir_path: &PathBuf,
    ) -> Result<(), Error> {
        let dir_name = dir_path.file_name().and_then(|name| name.to_str());

        if let Some(old_dir_name) = dir_name {
            let new_dir_name = old_dir_name.replace(&template_info.placeholder, &self.config.name);
            let new_dir_path = dir_path.with_file_name(&new_dir_name);

            if new_dir_name != old_dir_name {
                println!(
                    "Renaming {} -> {}",
                    dir_path.display(),
                    new_dir_path.display()
                );
                fs::rename(dir_path, new_dir_path).map_err(Error::RenameDir)?;
            }
        }

        Ok(())
    }
}

struct Paths {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

#[derive(Clone)]
pub enum Template {
    CounterTailwind,
    Custom(TemplateInfo),
}

#[derive(Clone)]
pub struct TemplateInfo {
    url: String,
    path: String,
    placeholder: String,
}

impl Template {
    pub fn info(&self) -> TemplateInfo {
        match self {
            Template::CounterTailwind => {
                // fmt
                TemplateInfo{
                    url: "https://github.com/polyester-web/polyester-templates/archive/refs/heads/main.zip".to_string(),
                    path: "counter-tailwind".to_string(),
                    placeholder: "myapp".to_string(),
                }
            }

            Template::Custom(info) => {
                // fmt
                info.clone()
            }
        }
    }
}
