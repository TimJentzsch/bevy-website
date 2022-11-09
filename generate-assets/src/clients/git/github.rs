use anyhow::bail;
use serde::Deserialize;

use super::{GitRemoteClient, GitRepositoryClient};

const BASE_URL: &str = "https://api.github.com";

#[derive(Deserialize)]
struct GithubContentResponse {
    encoding: String,
    content: String,
}

#[derive(Deserialize)]
struct GithubLicenseResponse {
    license: GithubLicenseLicense,
}

#[derive(Deserialize)]
struct GithubLicenseLicense {
    spdx_id: String,
}

#[derive(Debug, Clone)]
pub struct GithubClient {
    agent: ureq::Agent,
    token: String,
}

impl GithubClient {
    pub fn new(token: String) -> Self {
        let agent: ureq::Agent = ureq::AgentBuilder::new()
            .user_agent("bevy-website-generate-assets")
            .build();

        Self { agent, token }
    }
}

impl GitRemoteClient for GithubClient {
    type Client = GithubRepoClient;

    fn try_get_repository_client(&self, url: url::Url) -> anyhow::Result<Self::Client> {
        if let Some(host) = url.host_str() {
            if host != "github.com" {
                bail!("Not a GitHub repository");
            }
        } else {
            bail!("No host in URL");
        }

        let segments = url.path_segments().map(|c| c.collect::<Vec<_>>()).unwrap();
        let username = segments[0].to_string();
        let repository_name = segments[1].to_string();

        Ok(GithubRepoClient {
            client: self.clone(),
            username,
            repository_name,
        })
    }
}

pub struct GithubRepoClient {
    client: GithubClient,
    username: String,
    repository_name: String,
}

impl GitRepositoryClient for GithubRepoClient {
    fn try_get_file_content(&self, file_path: &str) -> anyhow::Result<String> {
        let response: GithubContentResponse = self
            .client
            .agent
            .get(&format!(
                "{BASE_URL}/repos/{username}/{repository_name}/contents/{file_path}",
                username = self.username,
                repository_name = self.repository_name
            ))
            .set("Accept", "application/json")
            .set("Authorization", &format!("Bearer {}", self.client.token))
            .call()?
            .into_json()?;

        if response.encoding == "base64" {
            let data = base64::decode(response.content.replace('\n', "").trim())?;
            Ok(String::from_utf8(data)?)
        } else {
            bail!("Content is not in base64");
        }
    }

    /// Technically, github supports multiple licenses, but the api only returns one
    fn try_get_license(&self) -> anyhow::Result<String> {
        let response: GithubLicenseResponse = self
            .client
            .agent
            .get(&format!(
                "{BASE_URL}/repos/{username}/{repository_name}/license",
                username = self.username,
                repository_name = self.repository_name
            ))
            .set("Accept", "application/json")
            .set("Authorization", &format!("Bearer {}", self.client.token))
            .call()?
            .into_json()?;

        Ok(response.license.spdx_id)
    }
}
