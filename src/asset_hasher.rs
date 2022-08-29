use crate::util::file_util;
use crate::ProjectInfo;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::ops::Deref;
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct Config {
    pub core_project_path: PathBuf,
    pub dist_path: PathBuf,
}

impl Config {
    pub fn from_project_info(project_info: &ProjectInfo) -> Self {
        Self {
            core_project_path: project_info.core_project_path.clone(),
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
}

impl AssetHasher {
    pub fn new(config: Config) -> AssetHasher {
        AssetHasher { config }
    }

    pub fn run(&self) -> Result<(), Error> {
        let rust_files = self.collect_rust_files(&self.config.core_project_path);
        let mut all_assets: HashSet<HashedAsset> = HashSet::new();

        rust_files
            .iter()
            .map(|file_path| {
                let assets = self.find_local_assets_in_file(file_path)?;
                let hashed_assets = assets
                    .into_iter()
                    .map(|asset| self.hash_asset(asset))
                    .collect::<Result<Vec<HashedAsset>, Error>>()?;

                let assets_set: HashSet<HashedAsset> = HashSet::from_iter(hashed_assets.clone());
                all_assets.extend(assets_set);

                self.update_uris_in_file(&file_path, hashed_assets)?;

                Ok(())
            })
            .collect::<Result<(), Error>>()?;

        all_assets
            .iter()
            .map(|asset| self.rename_asset(asset))
            .collect::<Result<(), Error>>()?;

        Ok(())
    }

    fn collect_rust_files(&self, path: &PathBuf) -> Vec<PathBuf> {
        let paths = WalkDir::new(path).into_iter().filter_map(|entry| {
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
        });

        paths
            .filter(|path| path.extension() == Some(OsStr::new("rs")))
            .collect()
    }

    fn find_local_assets_in_file(&self, file_path: &PathBuf) -> Result<Vec<Asset>, Error> {
        let content = fs::read_to_string(&file_path).map_err(Error::ReadFile)?;

        let link_uris = content
            .lines()
            .filter(|line| is_link_asset(line) && !has_nohash(line))
            .filter_map(extract_link_href);

        let script_uris = content
            .lines()
            .filter(|line| is_script_asset(line) && !has_nohash(line))
            .filter_map(extract_script_src);

        let assets = link_uris
            .chain(script_uris)
            .filter(|uri| is_local_uri(uri))
            .map(|uri| Asset {
                uri: uri.to_string(),
                path: self.config.dist_path.join(uri.trim_start_matches("/")),
            })
            .filter(|asset| asset.path.exists())
            .collect();

        Ok(assets)
    }

    fn hash_asset(&self, asset: Asset) -> Result<HashedAsset, Error> {
        let mut hasher = Sha256::new();
        let mut file = fs::File::open(&asset.path).map_err(Error::OpenAssetFile)?;
        io::copy(&mut file, &mut hasher).map_err(Error::HashAssetFile)?;
        let digest = hasher.finalize();

        let hashed_asset = HashedAsset {
            asset: asset,
            hash: data_encoding::HEXLOWER.encode(&digest),
        };

        Ok(hashed_asset)
    }

    fn update_uris_in_file(
        &self,
        file_path: &PathBuf,
        assets: Vec<HashedAsset>,
    ) -> Result<(), Error> {
        let old_file = file_util::read(&file_path).map_err(Error::ReadFile)?;

        let new_content = assets.iter().fold(old_file.content, |acc, asset| {
            println!(
                "Replacing uri {} -> {} in {}",
                asset.uri,
                asset.uri_with_hash(),
                file_path.display()
            );
            acc.replace(&asset.uri, &asset.uri_with_hash())
        });

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

fn is_link_asset(s: &str) -> bool {
    s.contains("link") && s.contains("href")
}

fn is_script_asset(s: &str) -> bool {
    s.contains("script") && s.contains("src")
}

fn has_nohash(s: &str) -> bool {
    s.contains("nohash")
}

fn extract_link_href(s: &str) -> Option<String> {
    extract_attribute_value(s, "href")
}

fn extract_script_src(s: &str) -> Option<String> {
    extract_attribute_value(s, "src")
}

fn is_local_uri(s: &str) -> bool {
    !s.starts_with("http")
}

fn extract_attribute_value(s: &str, name: &str) -> Option<String> {
    let quote_char = '"';
    let pattern = format!("{}={}", name, quote_char);
    let pattern_index = s.find(&pattern)?;
    let value_start_index = pattern_index + pattern.len();
    let value_length = s[value_start_index..].find(quote_char)?;
    let value_end_index = value_start_index + value_length;

    Some(s[value_start_index..value_end_index].to_string())
}

#[derive(Clone, Eq, PartialEq, Hash)]
struct Asset {
    uri: String,
    path: PathBuf,
}

#[derive(Clone, Eq, PartialEq, Hash)]
struct HashedAsset {
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
        let old_ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let new_ext = format!("{}.{}", self.short_hash(), old_ext);

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
