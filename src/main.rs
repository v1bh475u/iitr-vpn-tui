mod app;
mod config;
mod ui;
mod vpn;

use app::{Action, App};
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{env, io, panic, time::Duration};

fn main() {
    if let Err(error) = run() {
        eprintln!("iitr-vpn: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    if let Some(argument) = env::args().nth(1) {
        match argument.as_str() {
            "--disconnect" => {
                println!("Stopping any OpenConnect session using iitr-vpn0.");
                vpn::authorize_sudo()?;
                let stopped = vpn::disconnect_interface("iitr-vpn0")?;
                if stopped == 0 {
                    println!("No active IITR VPN session was found.");
                } else {
                    println!("Disconnect signal sent to {stopped} IITR VPN session(s).");
                }
            }
            "-h" | "--help" => print_help(),
            "-V" | "--version" => println!("iitr-vpn {}", env!("CARGO_PKG_VERSION")),
            _ => return Err(format!("unknown option '{argument}'; try --help")),
        }
        return Ok(());
    }

    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        previous_hook(info);
    }));

    let mut app = App::new();
    start_terminal()?;
    let result = event_loop(&mut app);
    let restore_result = restore_terminal();
    result.and(restore_result)
}

fn print_help() {
    println!(
        "iitr-vpn {}\n\nSecure terminal client for IIT Roorkee's AnyConnect VPN\n\n\
USAGE:\n    iitr-vpn [OPTION]\n\n\
OPTIONS:\n    --disconnect    Stop an active iitr-vpn0 tunnel\n\
    -h, --help      Print this help\n\
    -V, --version   Print version information",
        env!("CARGO_PKG_VERSION")
    );
}

fn event_loop(app: &mut App) -> Result<(), String> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|error| error.to_string())?;
    loop {
        terminal
            .draw(|frame| ui::draw(frame, app))
            .map_err(|error| error.to_string())?;
        if event::poll(Duration::from_millis(150)).map_err(|error| error.to_string())?
            && let Event::Key(key) = event::read().map_err(|error| error.to_string())?
            && key.kind == event::KeyEventKind::Press
        {
            match app.on_key(key) {
                Action::None => {}
                Action::Quit => return Ok(()),
                Action::AuthorizeAndConnect(request) => {
                    let authorization = authorize_outside_tui(
                        &mut terminal,
                        "IITR VPN needs administrator access to create the TUN interface.",
                    )?;
                    match authorization {
                        Ok(()) => app.start_after_authorization(request),
                        Err(error) => app.authorization_failed(error),
                    }
                }
                Action::AuthorizeAndDisconnect => {
                    let authorization = authorize_outside_tui(
                        &mut terminal,
                        "IITR VPN needs administrator access to stop the tunnel cleanly.",
                    )?;
                    match authorization {
                        Ok(()) => app.disconnect_after_authorization(),
                        Err(error) => app.disconnect_authorization_failed(error),
                    }
                }
            }
        }
        app.tick();
    }
}

fn authorize_outside_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    explanation: &str,
) -> Result<Result<(), String>, String> {
    restore_terminal()?;
    println!("{explanation}");
    let authorization = vpn::authorize_sudo();
    start_terminal()?;
    terminal.clear().map_err(|error| error.to_string())?;
    Ok(authorization)
}

fn start_terminal() -> Result<(), String> {
    enable_raw_mode().map_err(|error| error.to_string())?;
    execute!(io::stdout(), EnterAlternateScreen).map_err(|error| {
        let _ = disable_raw_mode();
        error.to_string()
    })
}

fn restore_terminal() -> Result<(), String> {
    let raw_result = disable_raw_mode();
    let screen_result = execute!(io::stdout(), LeaveAlternateScreen);
    raw_result
        .and(screen_result)
        .map_err(|error| error.to_string())
}
