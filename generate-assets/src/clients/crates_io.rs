use std::sync::Arc;

use anyhow::bail;

use super::{Metadata, MetadataAssetClient, MetadataClient};

pub type CratesIoDb = cratesio_dbdump_csvtab::rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct CratesioClient {
    db: Arc<CratesIoDb>,
}

impl MetadataClient for CratesioClient {
    type Client = CratesioCrateClient;

    fn try_get_repository_client(&self, url: url::Url) -> anyhow::Result<Self::Client> {
        if let Some(host) = url.host_str() {
            if host != "crates.io" {
                bail!("Not a crates.io link");
            }
        } else {
            bail!("No host in URL");
        }

        let segments = url.path_segments().map(|c| c.collect::<Vec<_>>()).unwrap();
        let crate_name = segments[1].to_string();

        Ok(CratesioCrateClient {
            client: self.clone(),
            crate_name,
        })
    }
}

pub struct CratesioCrateClient {
    client: CratesioClient,
    crate_name: String,
}

impl MetadataAssetClient for CratesioCrateClient {
    fn try_get_metadata(&self) -> anyhow::Result<Metadata> {
        get_metadata_from_crates_io_db(&self.client.db, &self.crate_name)
    }
}

/// Gets the required metadata from the crates.io database dump
fn get_metadata_from_crates_io_db(db: &CratesIoDb, crate_name: &str) -> anyhow::Result<Metadata> {
    if let Ok(metadata) = get_metadata_from_db_by_crate_name(db, crate_name) {
        Ok(metadata)
    } else if let Ok(metadata) =
        get_metadata_from_db_by_crate_name(db, &crate_name.replace('_', "-"))
    {
        Ok(metadata)
    } else {
        bail!("Failed to get data from crates.io db for {crate_name}")
    }
}

fn get_metadata_from_db_by_crate_name(
    db: &CratesIoDb,
    crate_name: &str,
) -> anyhow::Result<Metadata> {
    if let Some(Ok((_, _, license, _, deps))) =
        &cratesio_dbdump_lookup::get_rev_dependency(db, crate_name, "bevy")?.first()
    {
        let bevy_version = deps
            .as_ref()
            .ok()
            .and_then(|deps| deps.first())
            .map(|(version, _)| version.clone());

        Ok(Metadata {
            license: Some(license.clone()),
            bevy_version,
        })
    } else {
        bail!("Not found in crates.io db: {crate_name}")
    }
}
