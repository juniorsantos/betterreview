use std::{collections::BTreeMap, ffi::OsString, fmt, path::PathBuf, sync::Arc, time::Duration};

use semver::Version;

use crate::{
    domain::ProviderKind,
    process::{CommandOutput, CommandRunner, CommandSpec},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyStatus {
    Ready,
    Missing,
    Unsupported,
    AuthenticationRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyCheck {
    pub name: String,
    pub status: DependencyStatus,
    pub detected_version: Option<Version>,
    pub guidance: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub checks: Vec<DependencyCheck>,
}

impl DoctorReport {
    pub fn is_ready(&self) -> bool {
        self.checks
            .iter()
            .all(|check| check.status == DependencyStatus::Ready)
    }

    pub fn requirement(&self, name: &str) -> Option<&DependencyCheck> {
        self.checks.iter().find(|check| check.name == name)
    }
}

impl fmt::Display for DoctorReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for check in &self.checks {
            let status = match check.status {
                DependencyStatus::Ready => "ready",
                DependencyStatus::Missing => "missing",
                DependencyStatus::Unsupported => "unsupported",
                DependencyStatus::AuthenticationRequired => "authentication required",
            };
            match &check.detected_version {
                Some(version) => writeln!(
                    formatter,
                    "{}: {} ({}) - {}",
                    check.name, status, version, check.guidance
                )?,
                None => writeln!(formatter, "{}: {} - {}", check.name, status, check.guidance)?,
            }
        }
        Ok(())
    }
}

pub struct Doctor {
    runner: Arc<dyn CommandRunner>,
}

impl Doctor {
    pub fn new(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }

    pub async fn check(&self, provider: Option<ProviderKind>, host: Option<&str>) -> DoctorReport {
        let github = matches!(provider, Some(ProviderKind::GitHub) | None);
        let gitlab = matches!(provider, Some(ProviderKind::GitLab) | None);
        let (git, delta, gh, glab) = tokio::join!(
            self.check_version("git", &["--version"], Version::new(2, 39, 0)),
            self.check_version("delta", &["--version"], Version::new(0, 19, 0)),
            async {
                if github {
                    Some(
                        self.check_provider(
                            "gh",
                            host.unwrap_or("github.com"),
                            Version::new(2, 40, 0),
                        )
                        .await,
                    )
                } else {
                    None
                }
            },
            async {
                if gitlab {
                    Some(
                        self.check_provider(
                            "glab",
                            host.unwrap_or("gitlab.com"),
                            Version::new(1, 50, 0),
                        )
                        .await,
                    )
                } else {
                    None
                }
            },
        );
        let checks = [Some(git), Some(delta), gh, glab]
            .into_iter()
            .flatten()
            .collect();

        DoctorReport { checks }
    }

    async fn check_version(&self, name: &str, args: &[&str], minimum: Version) -> DependencyCheck {
        let output = match self.run(name, args).await {
            Ok(output) => output,
            Err(()) => return missing(name, minimum),
        };
        if output.status != 0 {
            return missing(name, minimum);
        }
        let version = parse_version(&output);
        match version {
            Some(version) if version >= minimum => DependencyCheck {
                name: name.to_owned(),
                status: DependencyStatus::Ready,
                detected_version: Some(version),
                guidance: "Ready".into(),
            },
            version => DependencyCheck {
                name: name.to_owned(),
                status: DependencyStatus::Unsupported,
                detected_version: version,
                guidance: format!("Upgrade {name} to {minimum} or newer"),
            },
        }
    }

    async fn check_provider(&self, name: &str, host: &str, minimum: Version) -> DependencyCheck {
        let version_check = self.check_version(name, &["--version"], minimum).await;
        if version_check.status != DependencyStatus::Ready {
            return version_check;
        }

        let auth_args = ["auth", "status", "--hostname", host];
        let (help, auth) = tokio::join!(
            self.run(name, &["api", "--help"]),
            self.run(name, &auth_args),
        );
        let help = match help {
            Ok(output) if output.status == 0 => output,
            _ => {
                return DependencyCheck {
                    name: name.to_owned(),
                    status: DependencyStatus::Unsupported,
                    detected_version: version_check.detected_version,
                    guidance: format!("{name} api --help is unavailable"),
                };
            }
        };
        let help_text = combined_output(&help);
        if !help_text.contains("--input") {
            return DependencyCheck {
                name: name.to_owned(),
                status: DependencyStatus::Unsupported,
                detected_version: version_check.detected_version,
                guidance: format!("Upgrade {name}; api --input - is required"),
            };
        }

        let authenticated = auth.is_ok_and(|output| output.status == 0);
        if !authenticated {
            return DependencyCheck {
                name: name.to_owned(),
                status: DependencyStatus::AuthenticationRequired,
                detected_version: version_check.detected_version,
                guidance: format!("Run: {name} auth login --hostname {host}"),
            };
        }

        version_check
    }

    async fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, ()> {
        self.runner
            .run(CommandSpec {
                program: PathBuf::from(program),
                args: args.iter().map(OsString::from).collect(),
                stdin: None,
                cwd: None,
                timeout: Duration::from_secs(60),
                env: BTreeMap::new(),
                env_remove: Vec::new(),
            })
            .await
            .map_err(|_| ())
    }
}

fn missing(name: &str, minimum: Version) -> DependencyCheck {
    DependencyCheck {
        name: name.to_owned(),
        status: DependencyStatus::Missing,
        detected_version: None,
        guidance: format!("Install {name} {minimum} or newer"),
    }
}

fn parse_version(output: &CommandOutput) -> Option<Version> {
    combined_output(output)
        .split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .filter(|candidate| candidate.matches('.').count() >= 2)
        .find_map(|candidate| Version::parse(candidate.trim_matches('.')).ok())
}

fn combined_output(output: &CommandOutput) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}
