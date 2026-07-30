use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum ProviderArg {
    GitHub,
    GitLab,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchRequest {
    Review {
        target: Option<String>,
        provider: Option<ProviderArg>,
        host: Option<String>,
        repository: Option<String>,
        remote: Option<String>,
    },
    Resume(Option<String>),
    Sessions,
    Doctor {
        provider: Option<ProviderArg>,
        host: Option<String>,
    },
    Completions(Shell),
}

#[derive(Debug, Parser)]
#[command(name = "betterreview", version, about)]
pub struct Cli {
    pub target: Option<String>,

    #[arg(long, value_enum)]
    pub provider: Option<ProviderArg>,

    #[arg(long)]
    pub host: Option<String>,

    #[arg(long = "repo")]
    pub repository: Option<String>,

    #[arg(long)]
    pub remote: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Resume {
        session_id: Option<String>,
    },
    Sessions,
    Doctor {
        #[arg(long, value_enum)]
        provider: Option<ProviderArg>,
        #[arg(long)]
        host: Option<String>,
    },
    /// Print a completion script for the given shell
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
}

impl Cli {
    pub fn launch_request(&self) -> LaunchRequest {
        match &self.command {
            Some(Command::Resume { session_id }) => LaunchRequest::Resume(session_id.clone()),
            Some(Command::Sessions) => LaunchRequest::Sessions,
            Some(Command::Completions { shell }) => LaunchRequest::Completions(*shell),
            Some(Command::Doctor { provider, host }) => LaunchRequest::Doctor {
                provider: *provider,
                host: host.clone(),
            },
            None => LaunchRequest::Review {
                target: self.target.clone(),
                provider: self.provider,
                host: self.host.clone(),
                repository: self.repository.clone(),
                remote: self.remote.clone(),
            },
        }
    }
}

pub fn completions(shell: Shell) -> String {
    let mut command = Cli::command();
    let name = command.get_name().to_owned();
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut command, name, &mut script);
    String::from_utf8_lossy(&script).into_owned()
}
