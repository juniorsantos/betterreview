use betterreview::cli::{Cli, LaunchRequest, ProviderArg};
use clap::Parser as _;

#[test]
fn parses_url_target_and_provider_override() {
    let cli = Cli::try_parse_from([
        "betterreview",
        "https://git.example.test/group/repo/-/merge_requests/42",
        "--provider",
        "gitlab",
        "--host",
        "git.example.test",
    ])
    .unwrap();

    assert_eq!(
        cli.launch_request(),
        LaunchRequest::Review {
            target: Some("https://git.example.test/group/repo/-/merge_requests/42".into()),
            provider: Some(ProviderArg::GitLab),
            host: Some("git.example.test".into()),
            repository: None,
        }
    );
}

#[test]
fn parses_resume_and_doctor() {
    let resume = Cli::try_parse_from(["betterreview", "resume", "github-example-7"]).unwrap();
    assert_eq!(
        resume.launch_request(),
        LaunchRequest::Resume(Some("github-example-7".into()))
    );

    let doctor = Cli::try_parse_from(["betterreview", "doctor", "--provider", "github"]).unwrap();
    assert_eq!(
        doctor.launch_request(),
        LaunchRequest::Doctor {
            provider: Some(ProviderArg::GitHub),
            host: None
        }
    );
}
