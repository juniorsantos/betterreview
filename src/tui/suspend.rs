use std::io;
#[cfg(unix)]
use std::io::Write;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
#[cfg(unix)]
use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
#[cfg(unix)]
use ratatui::backend::Backend;

pub(crate) fn is_requested(key: KeyEvent) -> bool {
    cfg!(unix)
        && key.kind != KeyEventKind::Release
        && key.code == KeyCode::Char('z')
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(unix)]
trait Lifecycle {
    fn disable_mouse(&mut self) -> io::Result<()>;
    fn restore_terminal(&mut self) -> io::Result<()>;
    fn show_resume_hint(&mut self) -> io::Result<()>;
    fn stop_process(&mut self) -> io::Result<()>;
    fn initialize_terminal(&mut self) -> io::Result<()>;
    fn enable_mouse(&mut self) -> io::Result<()>;
    fn clear_terminal(&mut self) -> io::Result<()>;
}

#[cfg(unix)]
fn run_with(lifecycle: &mut impl Lifecycle) -> io::Result<()> {
    lifecycle.disable_mouse()?;
    lifecycle.restore_terminal()?;
    lifecycle.show_resume_hint()?;
    lifecycle.stop_process()?;
    lifecycle.initialize_terminal()?;
    lifecycle.enable_mouse()?;
    lifecycle.clear_terminal()?;
    Ok(())
}

#[cfg(unix)]
struct SystemLifecycle<'a> {
    terminal: &'a mut ratatui::DefaultTerminal,
}

#[cfg(unix)]
impl Lifecycle for SystemLifecycle<'_> {
    fn disable_mouse(&mut self) -> io::Result<()> {
        crossterm::execute!(io::stdout(), DisableMouseCapture)
    }

    fn restore_terminal(&mut self) -> io::Result<()> {
        ratatui::try_restore()
    }

    fn show_resume_hint(&mut self) -> io::Result<()> {
        write_resume_hint(io::stdout().lock())
    }

    fn stop_process(&mut self) -> io::Result<()> {
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::TSTP)
            .map_err(io::Error::from)
    }

    fn initialize_terminal(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        crossterm::execute!(io::stdout(), EnterAlternateScreen)
    }

    fn enable_mouse(&mut self) -> io::Result<()> {
        crossterm::execute!(io::stdout(), EnableMouseCapture)
    }

    fn clear_terminal(&mut self) -> io::Result<()> {
        self.terminal.backend_mut().clear()?;
        self.terminal.swap_buffers();
        Ok(())
    }
}

#[cfg(unix)]
pub(crate) fn run(terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    run_with(&mut SystemLifecycle { terminal })
}

#[cfg(unix)]
fn write_resume_hint(mut output: impl Write) -> io::Result<()> {
    writeln!(output, "betterreview suspended — resume with fg")?;
    output.flush()
}

#[cfg(not(unix))]
pub(crate) fn run(_terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_z_requests_suspension_on_unix() {
        let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL);

        assert_eq!(is_requested(key), cfg!(unix));
    }

    #[test]
    fn plain_z_does_not_request_suspension() {
        let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);

        assert!(!is_requested(key));
    }

    #[test]
    fn ctrl_z_release_does_not_request_suspension() {
        let key = KeyEvent::new_with_kind(
            KeyCode::Char('z'),
            KeyModifiers::CONTROL,
            KeyEventKind::Release,
        );

        assert!(!is_requested(key));
    }

    #[cfg(unix)]
    #[test]
    fn restores_shell_before_stopping_and_terminal_after_resuming() {
        #[derive(Default)]
        struct Recorder(Vec<&'static str>);

        impl Lifecycle for Recorder {
            fn disable_mouse(&mut self) -> io::Result<()> {
                self.0.push("disable_mouse");
                Ok(())
            }

            fn restore_terminal(&mut self) -> io::Result<()> {
                self.0.push("restore_terminal");
                Ok(())
            }

            fn show_resume_hint(&mut self) -> io::Result<()> {
                self.0.push("show_resume_hint");
                Ok(())
            }

            fn stop_process(&mut self) -> io::Result<()> {
                self.0.push("stop_process");
                Ok(())
            }

            fn initialize_terminal(&mut self) -> io::Result<()> {
                self.0.push("initialize_terminal");
                Ok(())
            }

            fn enable_mouse(&mut self) -> io::Result<()> {
                self.0.push("enable_mouse");
                Ok(())
            }

            fn clear_terminal(&mut self) -> io::Result<()> {
                self.0.push("clear_terminal");
                Ok(())
            }
        }

        let mut recorder = Recorder::default();

        run_with(&mut recorder).unwrap();

        assert_eq!(
            recorder.0,
            [
                "disable_mouse",
                "restore_terminal",
                "show_resume_hint",
                "stop_process",
                "initialize_terminal",
                "enable_mouse",
                "clear_terminal",
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn resume_hint_names_the_shell_command() {
        let mut output = Vec::new();

        write_resume_hint(&mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "betterreview suspended — resume with fg\n"
        );
    }
}
