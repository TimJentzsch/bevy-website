use anyhow::{bail, Context};
use serde::Deserialize;

use super::{GitRemoteClient, GitRepositoryClient};

const BASE_URL: &str = "https://gitlab.com/api/v4/projects";

#[derive(Deserialize)]
pub struct GitlabProjectSearchResponse {
    pub id: usize,
    pub default_branch: String,
}

#[derive(Deserialize)]
struct GitlabContentResponse {
    encoding: String,
    content: String,
}

#[derive(Debug, Clone)]
pub struct GitlabClient {
    agent: ureq::Agent,
    // This is not currently used because we have so few assets using gitlab that we don't need it.
    _token: String,
}

impl GitlabClient {
    pub fn new(token: String) -> Self {
        let agent: ureq::Agent = ureq::AgentBuilder::new()
            .user_agent("bevy-website-generate-assets")
            .build();

        Self {
            agent,
            _token: token,
        }
    }

    /// Finds a list of repo based on their name
    /// Useful to get the repo id and default_branch
    pub fn search_project_by_name(
        &self,
        repository_name: &str,
    ) -> anyhow::Result<Vec<GitlabProjectSearchResponse>> {
        let reponse: Vec<GitlabProjectSearchResponse> = self
            .agent
            .get(&format!("{BASE_URL}?search={repository_name}"))
            .set("Accept", "application/json")
            // .set("Authorization", &format!("Bearer {}", self.token))
            .call()?
            .into_json()?;
        Ok(reponse)
    }
}

impl GitRemoteClient for GitlabClient {
    type Client = GitlabRepoClient;

    fn try_get_repository_client(&self, url: url::Url) -> anyhow::Result<Self::Client> {
        if let Some(host) = url.host_str() {
            if host != "gitlab.com" {
                bail!("Not a GitLab repository");
            }
        } else {
            bail!("No host in URL");
        }

        let segments = url.path_segments().map(|c| c.collect::<Vec<_>>()).unwrap();
        let repository_name = segments[1];

        let search_result = self.search_project_by_name(repository_name)?;

        let repo = search_result
            .first()
            .context("Failed to find gitlab repo")?;

        Ok(GitlabRepoClient {
            client: self.clone(),
            id: repo.id,
            default_branch: repo.default_branch,
        })
    }
}

pub struct GitlabRepoClient {
    client: GitlabClient,
    id: usize,
    default_branch: String,
}

impl GitRepositoryClient for GitlabRepoClient {
    fn try_get_file_content(&self, file_path: &str) -> anyhow::Result<String> {
        let reponse: GitlabContentResponse = self
            .client
            .agent
            .get(&format!(
                "{BASE_URL}/{id}/repository/files/{file_path}?ref={default_branch}",
                id = self.id,
                default_branch = self.default_branch
            ))
            .set("Accept", "application/json")
            // .set("Authorization", &format!("Bearer {}", self.token))
            .call()?
            .into_json()?;

        if reponse.encoding == "base64" {
            let data = base64::decode(reponse.content.replace('\n', "").trim())?;
            Ok(String::from_utf8(data)?)
        } else {
            bail!("Content is not in base64");
        }
    }

    fn try_get_license(&self) -> anyhow::Result<String> {
        Err(anyhow::anyhow!(
            "License fetching is not supported by GitLab."
        ))
    }
}
