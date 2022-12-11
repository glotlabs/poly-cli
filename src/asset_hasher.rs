use crate::util::file_util;
use crate::ProjectInfo;
use sha2::Digest;
use sha2::Sha256;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::ops::Deref;
use std::path;
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct Config {
    pub core_project_path_src: PathBuf,
    pub web_project_path_src: PathBuf,
    pub dist_path: PathBuf,
}

impl Config {
    pub fn from_project_info(project_info: &ProjectInfo) -> Self {
        Self {
            core_project_path_src: project_info.core_project_path_src(),
            web_project_path_src: project_info.web_project_path_src(),
            dist_path: project_info.dist_path.clone(),
        }
    }
}

pub struct AssetHasher {
    config: Config,
}

#[derive(Debug)]
pub enum Error {
    ReadFile(io::Error),
    OpenAssetFile(io::Error),
    HashAssetFile(io::Error),
    RenameAssetFile(io::Error),
    WriteSourceFile(io::Error),
    StripPathPrefix(path::StripPrefixError),
}

impl AssetHasher {
    pub fn new(config: Config) -> AssetHasher {
        AssetHasher { config }
    }

    pub fn collect_hashed_dist_assets(&self) -> Result<Vec<HashedAsset>, Error> {
        let dist_assets = self.collect_dist_assets()?;

        dist_assets
            .into_iter()
            .map(|asset| self.hash_asset(asset))
            .collect::<Result<Vec<HashedAsset>, Error>>()
    }

    pub fn update_uris_in_files(&self, assets: &Vec<HashedAsset>) -> Result<(), Error> {
        let rust_files = self.collect_files_by_ext(&self.config.core_project_path_src, "rs");
        let typescript_files = self.collect_files_by_ext(&self.config.web_project_path_src, "ts");

        let files = [rust_files, typescript_files].concat();

        for path in files {
            self.update_uris_in_file(&path, &assets)?;
        }

        Ok(())
    }

    pub fn rename_assets(&self, assets: &Vec<HashedAsset>) -> Result<(), Error> {
        assets
            .iter()
            .map(|asset| self.rename_asset(asset))
            .collect::<Result<(), Error>>()
    }

    fn collect_dist_assets(&self) -> Result<Vec<Asset>, Error> {
        let dist_files = self.collect_files(&self.config.dist_path);

        dist_files
            .into_iter()
            .map(|path| {
                let uri = self.get_dist_uri(&self.config.dist_path, &path)?;
                Ok(Asset { path, uri })
            })
            .collect()
    }

    fn get_dist_uri(&self, dist_path: &PathBuf, path: &PathBuf) -> Result<String, Error> {
        let rel_path = path
            .strip_prefix(dist_path)
            .map_err(Error::StripPathPrefix)?;

        Ok(format!("/{}", rel_path.to_string_lossy().to_string()))
    }

    fn collect_files_by_ext(&self, path: &PathBuf, extension: &str) -> Vec<PathBuf> {
        WalkDir::new(path)
            .into_iter()
            .filter_map(|entry| {
                match entry {
                    Ok(entry) => {
                        //fmt
                        Some(entry.path().to_path_buf())
                    }

                    Err(err) => {
                        eprintln!("Warning: Can't access file: {}", err);
                        None
                    }
                }
            })
            .filter(|path| path.extension() == Some(OsStr::new(extension)))
            .collect()
    }

    fn collect_files(&self, path: &PathBuf) -> Vec<PathBuf> {
        WalkDir::new(path)
            .into_iter()
            .filter_map(|entry| {
                match entry {
                    Ok(entry) => {
                        //fmt
                        Some(entry.path().to_path_buf())
                    }

                    Err(err) => {
                        eprintln!("Warning: Can't access file: {}", err);
                        None
                    }
                }
            })
            .filter(|path| path.is_file())
            .collect()
    }

    fn hash_asset(&self, asset: Asset) -> Result<HashedAsset, Error> {
        let mut hasher = Sha256::new();
        let mut file = fs::File::open(&asset.path).map_err(Error::OpenAssetFile)?;
        io::copy(&mut file, &mut hasher).map_err(Error::HashAssetFile)?;
        let digest = hasher.finalize();

        let hashed_asset = HashedAsset {
            asset,
            hash: data_encoding::HEXLOWER.encode(&digest),
        };

        Ok(hashed_asset)
    }

    fn update_uris_in_file(
        &self,
        file_path: &PathBuf,
        assets: &Vec<HashedAsset>,
    ) -> Result<(), Error> {
        let old_file = file_util::read(&file_path).map_err(Error::ReadFile)?;

        let new_content = old_file
            .content
            .lines()
            .map(|line| {
                if has_nohash(line) {
                    line.to_string()
                } else {
                    assets.iter().fold(line.to_string(), |acc, asset| {
                        if line.contains(&asset.uri) {
                            println!(
                                "Replacing uri {} -> {} in {}",
                                asset.uri,
                                asset.uri_with_hash(),
                                file_path.display()
                            );
                            acc.replace(&asset.uri, &asset.uri_with_hash())
                        } else {
                            acc
                        }
                    })
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let new_file = file_util::FileData {
            content: new_content,
            permissions: old_file.permissions,
        };

        file_util::write(&file_path, new_file).map_err(Error::WriteSourceFile)?;

        Ok(())
    }

    fn rename_asset(&self, asset: &HashedAsset) -> Result<(), Error> {
        println!(
            "Renaming asset {} -> {}",
            asset.path.display(),
            asset.path_with_hash().display()
        );
        fs::rename(&asset.path, &asset.path_with_hash()).map_err(Error::RenameAssetFile)
    }
}

fn has_nohash(s: &str) -> bool {
    s.contains("nohash")
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Asset {
    uri: String,
    path: PathBuf,
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct HashedAsset {
    asset: Asset,
    hash: String,
}

impl HashedAsset {
    fn uri_with_hash(&self) -> String {
        let mut uri = self.uri.clone();
        let dot_index = uri.rfind('.').unwrap_or(uri.len());
        let hash = format!(".{}", self.short_hash());
        uri.replace_range(dot_index..dot_index, &hash);

        uri
    }

    fn path_with_hash(&self) -> PathBuf {
        let path = &self.path;

        let new_ext = match path.extension() {
            Some(old_ext) => {
                // fmt
                format!("{}.{}", self.short_hash(), old_ext.to_string_lossy())
            }

            None => {
                // fmt
                self.short_hash()
            }
        };

        path.with_extension(new_ext)
    }

    fn short_hash(&self) -> String {
        self.hash[..7].to_string()
    }
}

impl Deref for HashedAsset {
    type Target = Asset;

    fn deref(&self) -> &Self::Target {
        &self.asset
    }
}
