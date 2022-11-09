use anyhow::{bail, Context};
use clients::crates_io::CratesioClient;
use clients::git::GithubClient;
use clients::git::GitlabClient;
use clients::MetadataClient;
use cratesio_dbdump_csvtab::CratesIODumpLoader;
use serde::Deserialize;
use std::{fs, path::PathBuf, str::FromStr};

pub mod clients;

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub name: String,
    pub link: String,
    pub description: String,
    pub order: Option<usize>,
    pub image: Option<String>,
    pub licenses: Option<Vec<String>>,
    pub bevy_versions: Option<Vec<String>>,

    // this field is not read from the toml file
    #[serde(skip)]
    pub original_path: Option<PathBuf>,
}

impl Asset {
    /// Parses a license string separated with OR into a Vec<String>
    fn set_license(&mut self, license: Option<String>) {
        if self.licenses.is_some() {
            return;
        }
        if let Some(license) = license {
            let licenses = license
                .split(" OR ")
                .map(|x| x.trim().to_string())
                .collect();
            self.licenses = Some(licenses);
        }
    }

    fn set_bevy_version(&mut self, version: Option<String>) {
        if self.bevy_versions.is_some() {
            return;
        }
        if let Some(version) = version {
            self.bevy_versions = Some(vec![version]);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub content: Vec<AssetNode>,
    pub template: Option<String>,
    pub header: Option<String>,
    pub order: Option<usize>,
    pub sort_order_reversed: bool,
}

#[derive(Debug, Clone)]
pub enum AssetNode {
    Section(Section),
    Asset(Asset),
}
impl AssetNode {
    pub fn name(&self) -> String {
        match self {
            AssetNode::Section(content) => content.name.clone(),
            AssetNode::Asset(content) => content.name.clone(),
        }
    }
    pub fn order(&self) -> usize {
        match self {
            AssetNode::Section(content) => content.order.unwrap_or(99999),
            AssetNode::Asset(content) => content.order.unwrap_or(99999),
        }
    }
}

fn visit_dirs(
    dir: PathBuf,
    section: &mut Section,
    crates_io_client: Option<&CratesioClient>,
    github_client: Option<&GithubClient>,
    gitlab_client: Option<&GitlabClient>,
) -> anyhow::Result<()> {
    if dir.is_file() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name().unwrap() == ".git" || path.file_name().unwrap() == ".github" {
            continue;
        }
        if path.is_dir() {
            let folder = path.file_name().unwrap();
            let (order, sort_order_reversed) = if path.join("_category.toml").exists() {
                let from_file: toml::Value =
                    toml::de::from_str(&fs::read_to_string(path.join("_category.toml")).unwrap())
                        .unwrap();
                (
                    from_file
                        .get("order")
                        .and_then(|v| v.as_integer())
                        .map(|v| v as usize),
                    from_file
                        .get("sort_order_reversed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                )
            } else {
                (None, false)
            };
            let mut new_section = Section {
                name: folder.to_str().unwrap().to_string(),
                content: vec![],
                template: None,
                header: None,
                order,
                sort_order_reversed,
            };
            visit_dirs(
                path.clone(),
                &mut new_section,
                crates_io_client,
                github_client,
                gitlab_client,
            )?;
            section.content.push(AssetNode::Section(new_section));
        } else {
            if path.file_name().unwrap() == "_category.toml"
                || path.extension().expect("file must have an extension") != "toml"
            {
                continue;
            }

            let mut asset: Asset = toml::from_str(&fs::read_to_string(&path).unwrap())?;
            asset.original_path = Some(path);

            if let Err(err) =
                get_extra_metadata(&mut asset, crates_io_client, github_client, gitlab_client)
            {
                // We don't want to stop execution here
                eprintln!("Failed to get metadata for {}", asset.name);
                eprintln!("ERROR: {err:?}");
            }

            section.content.push(AssetNode::Asset(asset));
        }
    }

    Ok(())
}

pub fn parse_assets(
    asset_dir: &str,
    crates_io_client: Option<&CratesioClient>,
    github_client: Option<&GithubClient>,
    gitlab_client: Option<&GitlabClient>,
) -> anyhow::Result<Section> {
    let mut asset_root_section = Section {
        name: "Assets".to_string(),
        content: vec![],
        template: Some("assets.html".to_string()),
        header: Some("Assets".to_string()),
        order: None,
        sort_order_reversed: false,
    };
    visit_dirs(
        PathBuf::from_str(asset_dir).unwrap(),
        &mut asset_root_section,
        crates_io_client,
        github_client,
        gitlab_client,
    )?;
    Ok(asset_root_section)
}

/// Tries to get bevy supported version and license information from various external sources
fn get_extra_metadata(
    asset: &mut Asset,
    crates_io_client: Option<&CratesioClient>,
    github_client: Option<&GithubClient>,
    gitlab_client: Option<&GitlabClient>,
) -> anyhow::Result<()> {
    println!("Getting extra metadata for {}", asset.name);

    let url = url::Url::parse(&asset.link)?;

    let metadata = match url.host_str() {
        Some("crates.io") if crates_io_client.is_some() => {
            if let Some(db) = crates_io_client {
                let crate_name = segments[1];
                Some(get_metadata_from_crates_io_db(db, crate_name)?)
            } else {
                None
            }
        }
        Some("github.com") => {
            if let Some(client) = github_client {
                let username = segments[0];
                let repository_name = segments[1];
                Some(get_metadata_from_git(client, username, repository_name)?)
            } else {
                None
            }
        }
        Some("gitlab.com") => {
            if let Some(client) = gitlab_client {
                let repository_name = segments[1];
                Some(get_metadata_from_gitlab(client, repository_name)?)
            } else {
                None
            }
        }
        None => None,
        _ => bail!("Unknown host: {}", asset.link),
    };

    if let Some((license, version)) = metadata {
        asset.set_license(license);
        asset.set_bevy_version(version);
    }

    Ok(())
}

/// Downloads the crates.io database dump and open a connection to the db
pub fn prepare_crates_db() -> anyhow::Result<CratesIoDb> {
    let cache_dir = {
        let mut current_dir = std::env::current_dir()?;
        current_dir.push("data");
        current_dir
    };

    if cache_dir.exists() {
        println!("Using crates.io data dump cache from: {:?}", cache_dir);
    } else {
        println!("Downloading crates.io data dump");
    }

    Ok(CratesIODumpLoader::default()
        .tables(&["crates", "dependencies", "versions"])
        .preload(true)
        .update()?
        .open_db()?)
}
