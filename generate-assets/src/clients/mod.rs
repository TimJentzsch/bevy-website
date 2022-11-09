pub mod crates_io;
pub mod git;

pub struct Metadata {
    pub license: Option<String>,
    pub bevy_version: Option<String>,
}

/// A client that can return metadata for an asset.
pub trait MetadataClient {
    type Client: MetadataAssetClient;

    fn try_get_repository_client(&self, url: url::Url) -> anyhow::Result<Self::Client>;
}

/// A metadata client for a specific asset.
pub trait MetadataAssetClient {
    fn try_get_metadata(&self) -> anyhow::Result<Metadata>;
}
