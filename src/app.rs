use crate::{
    config::Config,
    vpn::{self, ConnectionRequest, ConnectionState, VpnSession},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{collections::VecDeque, sync::mpsc, thread};
use zeroize::Zeroize;

const LOG_LIMIT: usize = 250;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field {
    Gateway,
    Username,
    Group,
    Password,
    SecondFactor,
}

impl Field {
    pub const ALL: [Self; 5] = [
        Self::Gateway,
        Self::Username,
        Self::Group,
        Self::Password,
        Self::SecondFactor,
    ];

    fn next(self, backwards: bool) -> Self {
        let index = Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0);
        let next = if backwards {
            (index + Self::ALL.len() - 1) % Self::ALL.len()
        } else {
            (index + 1) % Self::ALL.len()
        };
        Self::ALL[next]
    }
}

pub enum Action {
    None,
    Quit,
    AuthorizeAndConnect(ConnectionRequest),
    AuthorizeAndDisconnect,
}

pub struct App {
    pub config: Config,
    pub password: String,
    pub second_factor: String,
    pub focus: Field,
    pub logs: VecDeque<String>,
    pub session: Option<VpnSession>,
    pub state: ConnectionState,
    pub message: String,
    diagnostics_receiver: Option<mpsc::Receiver<Vec<(bool, String)>>>,
}

impl App {
    pub fn new() -> Self {
        let (config, startup_message) = match Config::load() {
            Ok(config) => (
                config,
                "Ready. Tab moves; c connects; r checks the system.".to_owned(),
            ),
            Err(error) => (Config::default(), error),
        };
        let mut app = Self {
            config,
            password: String::new(),
            second_factor: String::new(),
            focus: Field::Gateway,
            logs: VecDeque::new(),
            session: None,
            state: ConnectionState::Disconnected,
            message: startup_message,
            diagnostics_receiver: None,
        };
        app.log("IITR VPN TUI started; secrets are never saved.");
        app
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return self.request_quit();
        }
        match key.code {
            KeyCode::Tab => {
                self.focus = self.focus.next(key.modifiers.contains(KeyModifiers::SHIFT));
            }
            KeyCode::BackTab => self.focus = self.focus.next(true),
            KeyCode::Enter if matches!(self.focus, Field::Password | Field::SecondFactor) => {
                return self.request_connect();
            }
            KeyCode::Char('c') if key.modifiers.is_empty() && self.can_use_shortcuts() => {
                return self.request_connect();
            }
            KeyCode::Char('d') if key.modifiers.is_empty() && self.can_use_shortcuts() => {
                return self.request_disconnect();
            }
            KeyCode::Char('r') if key.modifiers.is_empty() && self.can_use_shortcuts() => {
                self.run_diagnostics();
            }
            KeyCode::Char('s') if key.modifiers.is_empty() && self.can_use_shortcuts() => {
                self.save_config();
            }
            KeyCode::Char('q') if key.modifiers.is_empty() && self.can_use_shortcuts() => {
                return self.request_quit();
            }
            KeyCode::Esc => {
                self.password.zeroize();
                self.second_factor.zeroize();
                self.message = "Secret fields cleared.".to_owned();
            }
            KeyCode::Backspace => {
                self.active_text_mut().pop();
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.active_text_mut().push(character);
            }
            _ => {}
        }
        Action::None
    }

    pub fn tick(&mut self) {
        if let Some(receiver) = self.diagnostics_receiver.take() {
            match receiver.try_recv() {
                Ok(results) => {
                    for (passed, label) in results {
                        self.log(format!("{} {label}", if passed { "[ok]" } else { "[!!]" }));
                    }
                    self.message = "System check finished; see the log.".to_owned();
                }
                Err(mpsc::TryRecvError::Empty) => self.diagnostics_receiver = Some(receiver),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.message = "System check stopped unexpectedly.".to_owned();
                }
            }
        }

        let Some(mut session) = self.session.take() else {
            return;
        };
        for line in session.drain_events() {
            self.log(line);
        }
        self.state = session.state;
        match session.poll_exit() {
            Ok(Some(status)) => {
                self.state = if status.success() {
                    ConnectionState::Disconnected
                } else {
                    ConnectionState::Failed
                };
                self.log(format!("openconnect exited with {status}"));
                self.message = if status.success() {
                    "VPN disconnected.".to_owned()
                } else {
                    "Connection failed; inspect the log.".to_owned()
                };
            }
            Ok(None) => self.session = Some(session),
            Err(error) => {
                self.state = ConnectionState::Failed;
                self.log(error);
                self.session = Some(session);
            }
        }
    }

    pub fn start_after_authorization(&mut self, request: ConnectionRequest) {
        match VpnSession::start(request) {
            Ok(session) => {
                self.state = ConnectionState::Connecting;
                self.session = Some(session);
                self.password.zeroize();
                self.second_factor.zeroize();
                self.message = "Authenticating with IITR…".to_owned();
                self.log("Starting openconnect (Cisco AnyConnect protocol).".to_owned());
            }
            Err(error) => {
                self.state = ConnectionState::Failed;
                self.message = error.clone();
                self.log(error);
            }
        }
    }

    pub fn authorization_failed(&mut self, error: String) {
        self.state = ConnectionState::Failed;
        self.message = error.clone();
        self.log(error);
    }

    pub fn disconnect_authorization_failed(&mut self, error: String) {
        if let Some(session) = &self.session {
            self.state = session.state;
        }
        self.message = format!("VPN is still active: {error}");
        self.log(self.message.clone());
    }

    pub fn field_text(&self, field: Field) -> &str {
        match field {
            Field::Gateway => &self.config.gateway,
            Field::Username => &self.config.username,
            Field::Group => &self.config.auth_group,
            Field::Password => &self.password,
            Field::SecondFactor => &self.second_factor,
        }
    }

    fn active_text_mut(&mut self) -> &mut String {
        match self.focus {
            Field::Gateway => &mut self.config.gateway,
            Field::Username => &mut self.config.username,
            Field::Group => &mut self.config.auth_group,
            Field::Password => &mut self.password,
            Field::SecondFactor => &mut self.second_factor,
        }
    }

    fn request_connect(&mut self) -> Action {
        if self.session.is_some() {
            self.message = "Disconnect the current session first.".to_owned();
            return Action::None;
        }
        if !vpn::command_exists("openconnect") {
            self.message = "openconnect is missing: yay -S --needed openconnect".to_owned();
            self.log("[!!] openconnect executable not found in PATH".to_owned());
            return Action::None;
        }
        let request = ConnectionRequest {
            gateway: self.config.gateway.trim().to_owned(),
            username: self.config.username.trim().to_owned(),
            auth_group: self.config.auth_group.trim().to_owned(),
            interface: self.config.interface.clone(),
            password: self.password.clone(),
            second_factor: self.second_factor.clone(),
        };
        if let Err(error) = vpn::validate_request(&request) {
            self.message = error.clone();
            self.log(error);
            return Action::None;
        }
        self.save_config();
        Action::AuthorizeAndConnect(request)
    }

    fn request_disconnect(&mut self) -> Action {
        if self.session.is_some() {
            Action::AuthorizeAndDisconnect
        } else {
            self.message = "There is no active VPN session.".to_owned();
            Action::None
        }
    }

    pub fn disconnect_after_authorization(&mut self) {
        let result = self.session.as_mut().map(VpnSession::disconnect);
        match result {
            Some(Ok(())) => {
                self.state = ConnectionState::Disconnecting;
                self.message = "Stopping the VPN…".to_owned();
                self.log("Sent SIGINT to openconnect.".to_owned());
            }
            Some(Err(error)) => {
                self.message = error.clone();
                self.log(error);
            }
            None => self.message = "There is no active VPN session.".to_owned(),
        }
    }

    fn request_quit(&mut self) -> Action {
        if self.session.is_some() {
            self.message = "Disconnect with d before quitting.".to_owned();
            Action::None
        } else {
            self.password.zeroize();
            self.second_factor.zeroize();
            Action::Quit
        }
    }

    fn save_config(&mut self) {
        match self.config.save() {
            Ok(path) => self.message = format!("Saved {} (no secrets).", path.display()),
            Err(error) => {
                self.message = error.clone();
                self.log(error);
            }
        }
    }

    fn run_diagnostics(&mut self) {
        if self.diagnostics_receiver.is_some() {
            self.message = "System check is already running.".to_owned();
            return;
        }
        let gateway = self.config.gateway.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(vpn::diagnostics(&gateway));
        });
        self.diagnostics_receiver = Some(receiver);
        self.message = "Checking dependencies, TUN, and gateway DNS…".to_owned();
    }

    fn can_use_shortcuts(&self) -> bool {
        !matches!(self.focus, Field::Gateway | Field::Username | Field::Group)
    }

    fn log(&mut self, message: impl Into<String>) {
        if self.logs.len() == LOG_LIMIT {
            self.logs.pop_front();
        }
        self.logs.push_back(message.into());
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.password.zeroize();
        self.second_factor.zeroize();
    }
}
