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

#[test]
fn parses_the_completions_subcommand() {
    let cli = Cli::try_parse_from(["betterreview", "completions", "zsh"]).unwrap();

    assert_eq!(
        cli.launch_request(),
        LaunchRequest::Completions(clap_complete::Shell::Zsh)
    );
}

#[test]
fn completions_rejects_an_unknown_shell() {
    assert!(Cli::try_parse_from(["betterreview", "completions", "csh"]).is_err());
}

#[test]
fn the_generated_script_mentions_the_binary_and_its_subcommands() {
    let script = betterreview::cli::completions(clap_complete::Shell::Bash);

    assert!(script.contains("betterreview"));
    for subcommand in ["resume", "sessions", "doctor", "completions"] {
        assert!(
            script.contains(subcommand),
            "{subcommand} missing from the bash script"
        );
    }
}

#[test]
fn every_shell_produces_a_script() {
    for shell in [
        clap_complete::Shell::Bash,
        clap_complete::Shell::Zsh,
        clap_complete::Shell::Fish,
    ] {
        assert!(
            !betterreview::cli::completions(shell).is_empty(),
            "{shell} produced nothing"
        );
    }
}
