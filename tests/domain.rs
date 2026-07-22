use betterreview::domain::{
    ChangeRequestKey, CommitOid, DiffPosition, DiffSelection, DiffSide, ProviderCapabilities,
    ProviderKind, RepoPath, ReviewOutcome, Support,
};

#[test]
fn request_key_is_stable_without_head_oid() {
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "acme/api".into(),
        number: 17,
    };
    assert_eq!(key.session_slug(), "github-github.com-acme-api-17");
}

#[test]
fn selection_keeps_canonical_positions() {
    let start = DiffPosition {
        path: RepoPath("src/lib.rs".into()),
        side: DiffSide::Right,
        line: 8,
        hunk: 1,
    };
    let end = DiffPosition {
        path: RepoPath("src/lib.rs".into()),
        side: DiffSide::Right,
        line: 10,
        hunk: 1,
    };
    assert_eq!(
        DiffSelection {
            start: start.clone(),
            end
        }
        .start,
        start
    );
}

#[test]
fn unsupported_capability_carries_visible_reason() {
    let caps = ProviderCapabilities::all_supported().with_request_changes(Support::Unsupported {
        reason: "GitLab request changes requires a supported server and tier".into(),
    });
    assert!(matches!(
        caps.for_outcome(ReviewOutcome::RequestChanges),
        Support::Unsupported { .. }
    ));
    assert_eq!(CommitOid("abc".into()).as_ref(), "abc");
}
