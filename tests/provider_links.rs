use betterreview::{
    domain::{ChangeRequestKey, CommitOid, ProviderKind, RepoPath},
    providers::ReviewLinks,
};

#[test]
fn github_links_target_the_review_commit_and_exact_file() {
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "owner/repo".into(),
        number: 42,
    };
    let links =
        ReviewLinks::new(&key, "https://github.com/owner/repo/pull/42").expect("valid links");

    assert_eq!(links.review_url(), "https://github.com/owner/repo/pull/42");
    assert_eq!(
        links.commit_url(&CommitOid("0123456789abcdef".into())),
        "https://github.com/owner/repo/commit/0123456789abcdef"
    );
    assert_eq!(
        links.file_url(&RepoPath("src/app/copy.rs".into())),
        "https://github.com/owner/repo/pull/42/files#diff-a70782b34ebd7a180a776ea72d6aff39c15f0a1d5160f2e463598ce02b119d7c"
    );
}

#[test]
fn gitlab_links_preserve_self_hosted_subgroups() {
    let key = ChangeRequestKey {
        provider: ProviderKind::GitLab,
        host: "git.acme.test".into(),
        repository: "group/platform/repo".into(),
        number: 17,
    };
    let links = ReviewLinks::new(
        &key,
        "https://git.acme.test/group/platform/repo/-/merge_requests/17",
    )
    .expect("valid links");

    assert_eq!(
        links.review_url(),
        "https://git.acme.test/group/platform/repo/-/merge_requests/17"
    );
    assert_eq!(
        links.commit_url(&CommitOid("0123456789abcdef".into())),
        "https://git.acme.test/group/platform/repo/-/commit/0123456789abcdef"
    );
    assert_eq!(
        links.file_url(&RepoPath("src/app/copy.rs".into())),
        "https://git.acme.test/group/platform/repo/-/merge_requests/17/diffs#c6a095d286de3508a4e1ef355dc340c60f33237d"
    );
}

#[test]
fn links_reject_untrusted_schemes_and_mismatched_review_paths() {
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "owner/repo".into(),
        number: 42,
    };

    assert!(ReviewLinks::new(&key, "javascript:alert(1)").is_none());
    assert!(ReviewLinks::new(&key, "https://github.com/other/repo/pull/42").is_none());
}
