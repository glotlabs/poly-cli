use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

pub struct FileData {
    pub content: String,
    pub permissions: fs::Permissions,
}

pub fn read(path: &PathBuf) -> Result<FileData, io::Error> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let mut content = String::new();

    file.read_to_string(&mut content)?;

    Ok(FileData {
        content,
        permissions: metadata.permissions(),
    })
}

pub fn write(path: &PathBuf, file_data: FileData) -> Result<(), io::Error> {
    let tmp_path = path.with_extension("tmp");

    // Make sure the file is closed before renaming (is this necessary?)
    {
        let mut tmp_file = File::create(&tmp_path)?;
        tmp_file.set_permissions(file_data.permissions)?;
        tmp_file.write_all(file_data.content.as_bytes())?;
    }

    fs::rename(&tmp_path, path)?;

    Ok(())
}
