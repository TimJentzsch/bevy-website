pub mod github;
pub mod gitlab;

/// A client for a remote provider (e.g. GitHub or GitLab).
pub trait GitRemoteClient {
    type Client: GitRepositoryClient;

    /// Try to get a client for the repository with the given URL.
    ///
    /// Gives an error when the URL doesn't match this remote provider.
    fn try_get_repository_client(&self, url: url::Url) -> anyhow::Result<Self::Client>;
}

/// A client for a specific repository.
pub trait GitRepositoryClient {
    /// Try the content of the given file.
    fn try_get_file_content(&self, file_path: &str) -> anyhow::Result<String>;

    /// Try to get the license of the repository (via the API).
    fn try_get_license(&self) -> anyhow::Result<String>;
}
