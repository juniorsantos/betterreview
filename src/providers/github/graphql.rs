pub const SNAPSHOT_QUERY: &str = r#"
query ReviewSnapshot($owner: String!, $name: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      number title url baseRefOid headRefOid
      author { login }
      files(first: 100) { nodes { path viewerViewedState } }
      reviewThreads(first: 100, after: $cursor) {
        nodes {
          id path isResolved isOutdated
          comments(first: 100) {
            nodes {
              id body line originalLine diffSide viewerDidAuthor
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

pub const BLOB_QUERY: &str = r#"
query ReviewBlob($owner: String!, $name: String!, $revisionPath: String!) {
  repository(owner: $owner, name: $name) {
    object(expression: $revisionPath) { ... on Blob { oid } }
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
