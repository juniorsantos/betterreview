use crate::domain::{ChangeRequestKey, ProviderKind};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUrl {
    pub host: String,
    pub repository: String,
}

#[derive(Debug, Error)]
pub enum RemoteUrlError {
    #[error("invalid remote or request URL: {0}")]
    Invalid(String),
}

pub fn parse_change_request_url(input: &str) -> Result<ChangeRequestKey, RemoteUrlError> {
    let url = url::Url::parse(input).map_err(|_| RemoteUrlError::Invalid(input.into()))?;
    let host = url
        .host_str()
        .ok_or_else(|| RemoteUrlError::Invalid(input.into()))?
        .to_owned();
    let segments = url
        .path_segments()
        .ok_or_else(|| RemoteUrlError::Invalid(input.into()))?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.len() == 4 && segments[2] == "pull" {
        return change_request_key(
            ProviderKind::GitHub,
            host,
            segments[..2].join("/"),
            segments[3],
            input,
        );
    }

    if segments.len() >= 4
        && segments[segments.len() - 3] == "-"
        && segments[segments.len() - 2] == "merge_requests"
    {
        return change_request_key(
            ProviderKind::GitLab,
            host,
            segments[..segments.len() - 3].join("/"),
            segments[segments.len() - 1],
            input,
        );
    }

    Err(RemoteUrlError::Invalid(input.into()))
}

pub fn parse_remote_url(input: &str) -> Result<RemoteUrl, RemoteUrlError> {
    let input = input.trim();
    if let Some((left, repository)) = input.split_once(':') {
        if !input.contains("://") && !repository.starts_with('/') {
            let host = left.rsplit_once('@').map_or(left, |(_, host)| host);
            return remote_url(host, repository, input);
        }
    }

    let url = url::Url::parse(input).map_err(|_| RemoteUrlError::Invalid(input.into()))?;
    let host = url
        .host_str()
        .ok_or_else(|| RemoteUrlError::Invalid(input.into()))?;
    remote_url(host, url.path(), input)
}

fn remote_url(host: &str, repository: &str, input: &str) -> Result<RemoteUrl, RemoteUrlError> {
    let repository = repository
        .trim_matches('/')
        .strip_suffix(".git")
        .unwrap_or(repository.trim_matches('/'));
    if host.is_empty() || repository.is_empty() {
        return Err(RemoteUrlError::Invalid(input.into()));
    }
    Ok(RemoteUrl {
        host: host.into(),
        repository: repository.into(),
    })
}

fn change_request_key(
    provider: ProviderKind,
    host: String,
    repository: String,
    number: &str,
    input: &str,
) -> Result<ChangeRequestKey, RemoteUrlError> {
    let number = number
        .parse()
        .map_err(|_| RemoteUrlError::Invalid(input.into()))?;
    Ok(ChangeRequestKey {
        provider,
        host,
        repository,
        number,
    })
}
