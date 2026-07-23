pub const SNAPSHOT_QUERY: &str = r#"
query ReviewSnapshot($owner: String!, $name: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      number title url baseRefOid headRefOid
      author { login }
      files(first: 100) { nodes { path viewerViewedState } }
      reviewThreads(first: 100, after: $cursor) {
        nodes {
          id path isResolved isOutdated diffSide
          comments(first: 100) {
            nodes {
              id body line originalLine viewerDidAuthor
              author { login }
              pullRequestReview { state }
            }
          }
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
"#;

pub const DISCOVER_QUERY: &str = r#"
query DiscoverReview($owner: String!, $name: String!, $head: String!) {
  repository(owner: $owner, name: $name) {
    pullRequests(first: 2, states: OPEN, headRefName: $head) { nodes { number } }
  }
}
"#;

pub const LIST_OPEN_QUERY: &str = r#"
query ListOpen($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    pullRequests(states: OPEN, first: 50, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes { number title isDraft updatedAt headRefName url author { login } }
    }
  }
}
"#;
