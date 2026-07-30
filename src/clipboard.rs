use crate::process::{CommandOutput, CommandRunner, CommandSpec};
use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Default)]
pub(crate) struct Clipboard {
    native: Option<arboard::Clipboard>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardBackend {
    Tmux,
    Wayland,
    Native,
    Osc52,
}

#[derive(Debug, Clone, Copy)]
struct ClipboardEnvironment {
    tmux: bool,
    ssh: bool,
    wayland: bool,
}

impl Clipboard {
    pub async fn copy(&mut self, runner: &dyn CommandRunner, content: &str) -> Result<(), String> {
        match select_backend(ClipboardEnvironment::current()) {
            ClipboardBackend::Tmux => copy_with_tmux(runner, content).await,
            ClipboardBackend::Wayland => self.copy_native(content, "Wayland"),
            ClipboardBackend::Native => self.copy_native(content, "system"),
            ClipboardBackend::Osc52 => copy_with_osc52(content),
        }
    }

    fn copy_native(&mut self, content: &str, backend: &str) -> Result<(), String> {
        if self.native.is_none() {
            self.native = Some(
                arboard::Clipboard::new()
                    .map_err(|error| format!("{backend} clipboard unavailable: {error}"))?,
            );
        }
        self.native
            .as_mut()
            .expect("clipboard initialized above")
            .set_text(content)
            .map_err(|error| format!("{backend} clipboard copy failed: {error}"))
    }
}

impl ClipboardEnvironment {
    fn current() -> Self {
        Self {
            tmux: environment_is_set("TMUX"),
            ssh: environment_is_set("SSH_TTY") || environment_is_set("SSH_CONNECTION"),
            wayland: environment_is_set("WAYLAND_DISPLAY"),
        }
    }
}

fn select_backend(environment: ClipboardEnvironment) -> ClipboardBackend {
    if environment.ssh && environment.tmux {
        ClipboardBackend::Tmux
    } else if environment.ssh {
        ClipboardBackend::Osc52
    } else if environment.wayland {
        ClipboardBackend::Wayland
    } else {
        ClipboardBackend::Native
    }
}

async fn copy_with_tmux(runner: &dyn CommandRunner, content: &str) -> Result<(), String> {
    let output = runner
        .run(tmux_command(content))
        .await
        .map_err(|error| format!("tmux clipboard unavailable: {error}"))?;
    if output.status == 0 {
        Ok(())
    } else {
        Err(command_failure("tmux clipboard copy failed", &output))
    }
}

fn tmux_command(content: &str) -> CommandSpec {
    CommandSpec {
        program: PathBuf::from("tmux"),
        args: ["load-buffer", "-w", "-"]
            .into_iter()
            .map(OsString::from)
            .collect(),
        stdin: Some(content.as_bytes().to_vec()),
        cwd: None,
        timeout: Duration::from_secs(5),
        env: BTreeMap::new(),
        env_remove: Vec::new(),
    }
}

fn command_failure(prefix: &str, output: &CommandOutput) -> String {
    let detail = String::from_utf8_lossy(&output.stderr);
    let detail = detail.trim();
    if detail.is_empty() {
        format!("{prefix} (exit {})", output.status)
    } else {
        format!("{prefix}: {detail}")
    }
}

fn copy_with_osc52(content: &str) -> Result<(), String> {
    let stdout = io::stdout();
    write_osc52(&mut stdout.lock(), content)
        .map_err(|error| format!("OSC 52 clipboard copy failed: {error}"))
}

fn environment_is_set(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn write_osc52(writer: &mut impl Write, content: &str) -> io::Result<()> {
    writer.write_all(osc52_sequence(content).as_bytes())?;
    writer.flush()
}

fn osc52_sequence(content: &str) -> String {
    format!("\u{1b}]52;c;{}\u{7}", encode_base64(content.as_bytes()))
}

fn encode_base64(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);

        encoded.push(BASE64[(first >> 2) as usize] as char);
        encoded.push(BASE64[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            BASE64[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            BASE64[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc52_sequence_base64_encodes_unicode_code() {
        assert_eq!(osc52_sequence("ação"), "\u{1b}]52;c;YcOnw6Nv\u{7}");
    }

    #[test]
    fn writer_receives_the_complete_osc52_sequence() {
        let mut output = Vec::new();

        write_osc52(&mut output, "hello").unwrap();

        assert_eq!(output, b"\x1b]52;c;aGVsbG8=\x07");
    }

    #[test]
    fn remote_tmux_uses_its_clipboard_bridge() {
        let environment = ClipboardEnvironment {
            tmux: true,
            ssh: true,
            wayland: false,
        };

        assert_eq!(select_backend(environment), ClipboardBackend::Tmux);
    }

    #[test]
    fn local_wayland_inside_tmux_still_uses_the_native_wayland_backend() {
        let environment = ClipboardEnvironment {
            tmux: true,
            ssh: false,
            wayland: true,
        };

        assert_eq!(select_backend(environment), ClipboardBackend::Wayland);
    }

    #[test]
    fn local_wayland_uses_the_native_wayland_backend() {
        let environment = ClipboardEnvironment {
            tmux: false,
            ssh: false,
            wayland: true,
        };

        assert_eq!(select_backend(environment), ClipboardBackend::Wayland);
    }

    #[test]
    fn remote_shell_without_tmux_keeps_osc52() {
        let environment = ClipboardEnvironment {
            tmux: false,
            ssh: true,
            wayland: false,
        };

        assert_eq!(select_backend(environment), ClipboardBackend::Osc52);
    }

    #[test]
    fn regular_local_terminal_uses_the_native_clipboard() {
        let environment = ClipboardEnvironment {
            tmux: false,
            ssh: false,
            wayland: false,
        };

        assert_eq!(select_backend(environment), ClipboardBackend::Native);
    }

    #[test]
    fn tmux_receives_clipboard_text_through_stdin() {
        let command = tmux_command("text with $() and café");

        assert_eq!(command.program, std::path::PathBuf::from("tmux"));
        assert_eq!(
            command.args,
            ["load-buffer", "-w", "-"]
                .into_iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            command.stdin.as_deref(),
            Some("text with $() and café".as_bytes())
        );
    }

    #[test]
    fn tmux_failure_reports_the_command_error() {
        let output = CommandOutput {
            status: 1,
            stdout: Vec::new(),
            stderr: b"clipboard forwarding is disabled\n".to_vec(),
        };

        assert_eq!(
            command_failure("tmux clipboard copy failed", &output),
            "tmux clipboard copy failed: clipboard forwarding is disabled"
        );
    }
}
