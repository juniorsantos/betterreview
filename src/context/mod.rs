mod git;
mod remote_url;
mod resolver;

pub use remote_url::{RemoteUrl, RemoteUrlError, parse_change_request_url, parse_remote_url};
pub use resolver::{
    ContextError, ContextResolver, DiscoveryInput, ResolveRequest, ResolvedContext,
};
