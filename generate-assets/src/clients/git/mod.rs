mod github;
mod gitlab;

use anyhow::Context;
pub use github::*;
pub use gitlab::*;

use super::{Metadata, MetadataAssetClient, MetadataClient};

/// A client for a remote provider (e.g. GitHub or GitLab).
pub trait GitRemoteClient {
    type Client: GitRepositoryClient;

    /// Try to get a client for the repository with the given URL.
    ///
    /// Gives an error when the URL doesn't match this remote provider.
    fn try_get_repository_client(&self, url: url::Url) -> anyhow::Result<Self::Client>;
}

impl<C> MetadataClient for C
where
    C: GitRemoteClient,
{
    type Client = <C as GitRemoteClient>::Client;

    fn try_get_repository_client(&self, url: url::Url) -> anyhow::Result<Self::Client> {
        GitRemoteClient::try_get_repository_client(self, url)
    }
}

/// A client for a specific repository.
pub trait GitRepositoryClient {
    /// Try the content of the given file.
    fn try_get_file_content(&self, file_path: &str) -> anyhow::Result<String>;

    /// Try to get the license of the repository (via the API).
    fn try_get_license(&self) -> anyhow::Result<String>;
}

impl<C> MetadataAssetClient for C
where
    C: GitRepositoryClient,
{
    fn try_get_metadata(&self) -> anyhow::Result<Metadata> {
        let cargo_toml_content = self
            .try_get_file_content("Cargo.toml")
            .context("Failed to get Cargo.toml")?;

        let cargo_manifest = toml::from_str::<cargo_toml::Manifest>(&cargo_toml_content)?;

        Ok(Metadata {
            license: get_license(&cargo_manifest),
            bevy_version: get_bevy_version(&cargo_manifest),
        })
    }
}

/// Gets the license from a Cargo.toml file
/// Tries to emulate crates.io behaviour
fn get_license(cargo_manifest: &cargo_toml::Manifest) -> Option<String> {
    // Get the license from the package information
    if let Some(cargo_toml::Package {
        license,
        license_file,
        ..
    }) = &cargo_manifest.package
    {
        if let Some(license) = license {
            Some(license.clone())
        } else {
            license_file.as_ref().map(|_| String::from("non-standard"))
        }
    } else {
        None
    }
}

/// Find any dep that starts with bevy and get the version
/// This makes sure to handle all the bevy_* crates
fn get_bevy_version(cargo_manifest: &cargo_toml::Manifest) -> Option<String> {
    cargo_manifest
        .dependencies
        .keys()
        .find(|k| k.starts_with("bevy"))
        .and_then(|key| {
            cargo_manifest
                .dependencies
                .get(key)
                .and_then(get_bevy_dependency_version)
        })
}

/// Gets the bevy version from the dependency list
/// Returns the version number if available.
/// If is is a git dependency, return either "main" or "git" for anything that isn't "main".
fn get_bevy_dependency_version(dep: &cargo_toml::Dependency) -> Option<String> {
    match dep {
        cargo_toml::Dependency::Simple(version) => Some(version.to_string()),
        cargo_toml::Dependency::Detailed(detail) => {
            if let Some(version) = &detail.version {
                Some(version.to_string())
            } else if detail.git.is_some() {
                if detail.branch == Some(String::from("main")) {
                    Some(String::from("main"))
                } else {
                    Some(String::from("git"))
                }
            } else {
                None
            }
        }
    }
}
