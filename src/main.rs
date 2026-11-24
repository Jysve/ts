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
    let command = match handle_arguments()? {
        Some(command) => command,
        None => return Ok(()),
    };

    let data_location = storage::data_location()?;
    let store = storage::load(&data_location)?;
    if command == Command::List {
        print_incomplete(&store);
        return Ok(());
    }

    let mut app = App::new(store, data_location, Local::now().date_naive());
    let mut terminal = TerminalSession::new()?;
    terminal.run(&mut app)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Command {
    Interactive,
    List,
}

fn print_incomplete(store: &model::Store) {
    let mut found = false;
    for date in store.days.keys().copied() {
        for todo in store
            .todos_on(date)
            .into_iter()
            .filter(|todo| !todo.completed)
        {
            println!("{date} [{:<6}] {}", todo.priority.label(), todo.title);
            found = true;
        }
    }
    if !found {
        println!("No incomplete todos.");
    }
}

fn handle_arguments() -> Result<Option<Command>> {
    let mut arguments = env::args_os().skip(1);
    let Some(argument) = arguments.next() else {
        return Ok(Some(Command::Interactive));
    };

    match argument.to_str() {
        Some("-h" | "--help") => {
            println!(
                "ts — interactive daily todos\n\n\
                 Usage:\n\
                   ts          Open the interactive interface\n\
                   ts --list   List all incomplete todos\n\n\
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
                   $XDG_CONFIG_HOME/ts, or ~/.config/ts\n\
                   Each date is stored as TOML under hashed month/day names.\n\
                   Existing JSON data is migrated without deleting the old file.\n\
                   Set TS_DATA_DIR to use a different directory.\n\
                   TS_DATA_FILE retains the legacy single-file JSON mode."
            );
            Ok(None)
        }
        Some("-V" | "--version") => {
            println!("ts {}", env!("CARGO_PKG_VERSION"));
            Ok(None)
        }
        Some("--list") => Ok(Some(Command::List)),
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
