mod app;
mod model;
mod storage;
mod ui;

use std::{env, io};

use anyhow::{Result, bail};
use app::App;
use chrono::Local;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

fn main() -> Result<()> {
    if handle_arguments()? {
        return Ok(());
    }

    let data_path = storage::data_file()?;
    let store = storage::load(&data_path)?;
    let mut app = App::new(store, data_path, Local::now().date_naive());
    let mut terminal = TerminalSession::new()?;
    terminal.run(&mut app)
}

fn handle_arguments() -> Result<bool> {
    let mut arguments = env::args_os().skip(1);
    let Some(argument) = arguments.next() else {
        return Ok(false);
    };

    match argument.to_str() {
        Some("-h" | "--help") => {
            println!(
                "ts — interactive daily todos\n\n\
                 Usage: ts\n\n\
                 The current date is selected on startup. Todos are saved after every change.\n\n\
                 Keys:\n\
                   ↑/↓ or J/K    Select a todo\n\
                   A or N        Add a todo and choose its priority\n\
                   Space/Enter   Toggle completion\n\
                   E             Edit the selected todo\n\
                   D             Open and edit details (Ctrl+S saves)\n\
                   1/2/3         Set High/Medium/Low priority\n\
                   ←/→           Previous/next date\n\
                   C             Open the calendar\n\
                   T             Go to today\n\
                   X             Delete the selected todo\n\
                   Q or Esc      Quit\n\n\
                 Data location:\n\
                   $XDG_CONFIG_HOME/ts/todos.json, or ~/.config/ts/todos.json\n\
                   Existing data is migrated from the previous data directory.\n\
                   Set TS_DATA_FILE to use a different file."
            );
            Ok(true)
        }
        Some("-V" | "--version") => {
            println!("ts {}", env!("CARGO_PKG_VERSION"));
            Ok(true)
        }
        _ => {
            bail!(
                "Unknown argument: {}\nTry 'ts --help' for usage.",
                argument.to_string_lossy()
            );
        }
    }
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalSession {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }

        let backend = CrosstermBackend::new(stdout);
        match Terminal::new(backend) {
            Ok(terminal) => Ok(Self { terminal }),
            Err(error) => {
                let _ = disable_raw_mode();
                let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
                Err(error.into())
            }
        }
    }

    fn run(&mut self, app: &mut App) -> Result<()> {
        loop {
            self.terminal.draw(|frame| ui::draw(frame, app))?;
            if let Event::Key(key) = event::read()?
                && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            {
                app.handle_key(key)?;
            }

            if app.should_quit {
                break;
            }
        }
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
    }
}
