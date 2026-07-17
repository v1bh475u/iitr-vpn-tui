use std::{
    env,
    ffi::OsString,
    fs,
    io::{BufRead, BufReader, Write},
    net::ToSocketAddrs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};
use url::Url;
use zeroize::Zeroizing;

pub struct ConnectionRequest {
    pub gateway: String,
    pub username: String,
    pub auth_group: String,
    pub interface: String,
    pub password: String,
    pub second_factor: String,
}

impl Drop for ConnectionRequest {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.password.zeroize();
        self.second_factor.zeroize();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Failed,
}

impl ConnectionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "DISCONNECTED",
            Self::Connecting => "CONNECTING",
            Self::Connected => "CONNECTED",
            Self::Disconnecting => "DISCONNECTING",
            Self::Failed => "FAILED",
        }
    }
}

#[derive(Debug)]
pub enum SessionEvent {
    Output(String),
}

pub struct VpnSession {
    child: Child,
    _stdin: ChildStdin,
    receiver: Receiver<SessionEvent>,
    interface: String,
    pub state: ConnectionState,
}

impl VpnSession {
    pub fn start(mut request: ConnectionRequest) -> Result<Self, String> {
        validate_request(&request)?;
        let interface = request.interface.clone();
        let mut command = Command::new("sudo");
        command
            .args(openconnect_args(&request))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);

        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start sudo/openconnect: {error}"))?;
        let mut child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| "openconnect stdin was unavailable".to_owned())?;

        // openconnect consumes these as authentication form responses. They never
        // appear in argv, the environment, the configuration, or our log pane.
        let password = Zeroizing::new(std::mem::take(&mut request.password));
        child_stdin
            .write_all(password.as_bytes())
            .and_then(|()| child_stdin.write_all(b"\n"))
            .map_err(|error| format!("could not send credentials to openconnect: {error}"))?;
        let second_factor = Zeroizing::new(std::mem::take(&mut request.second_factor));
        if !second_factor.is_empty() {
            child_stdin
                .write_all(second_factor.as_bytes())
                .and_then(|()| child_stdin.write_all(b"\n"))
                .map_err(|error| format!("could not send the second factor: {error}"))?;
        }
        child_stdin
            .flush()
            .map_err(|error| format!("could not flush credentials: {error}"))?;

        let (sender, receiver) = mpsc::channel();
        if let Some(stdout) = child.stdout.take() {
            spawn_reader(stdout, sender.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_reader(stderr, sender);
        }

        Ok(Self {
            child,
            _stdin: child_stdin,
            receiver,
            interface,
            state: ConnectionState::Connecting,
        })
    }

    pub fn drain_events(&mut self) -> Vec<String> {
        let mut output = Vec::new();
        while let Ok(SessionEvent::Output(line)) = self.receiver.try_recv() {
            self.observe(&line);
            output.push(line);
        }
        output
    }

    pub fn poll_exit(&mut self) -> Result<Option<ExitStatus>, String> {
        self.child
            .try_wait()
            .map_err(|error| format!("could not query openconnect: {error}"))
    }

    pub fn disconnect(&mut self) -> Result<(), String> {
        self.state = ConnectionState::Disconnecting;
        match disconnect_interface(&self.interface)? {
            0 => {
                // Authentication may have been cancelled before openconnect was
                // visible in /proc. Stop sudo's original process as a fallback.
                let status = Command::new("sudo")
                    .args(["-n", "kill", "-INT", &self.child.id().to_string()])
                    .status()
                    .map_err(|error| format!("could not cancel openconnect: {error}"))?;
                if status.success() {
                    Ok(())
                } else {
                    Err("disconnect command failed".to_owned())
                }
            }
            _ => Ok(()),
        }
    }

    fn observe(&mut self, line: &str) {
        let lower = line.to_ascii_lowercase();
        if lower.contains("cstp connected")
            || lower.contains("esp session established")
            || lower.contains("connected as ")
        {
            self.state = ConnectionState::Connected;
        } else if lower.contains("authentication failed")
            || lower.contains("login failed")
            || lower.contains("failed to open")
        {
            self.state = ConnectionState::Failed;
        }
    }
}

fn openconnect_args(request: &ConnectionRequest) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("-n"),
        OsString::from("openconnect"),
        OsString::from("--protocol=anyconnect"),
        OsString::from("--passwd-on-stdin"),
        OsString::from("--timestamp"),
        OsString::from(format!("--user={}", request.username)),
        OsString::from(format!("--interface={}", request.interface)),
    ];
    if !request.auth_group.trim().is_empty() {
        arguments.push(OsString::from(format!(
            "--authgroup={}",
            request.auth_group.trim()
        )));
    }
    arguments.push(OsString::from(&request.gateway));
    arguments
}

fn spawn_reader<R>(reader: R, sender: Sender<SessionEvent>)
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            match line {
                Ok(line) => {
                    if sender.send(SessionEvent::Output(line)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(SessionEvent::Output(format!("output error: {error}")));
                    break;
                }
            }
        }
    });
}

pub fn validate_request(request: &ConnectionRequest) -> Result<(), String> {
    let url = Url::parse(request.gateway.trim())
        .map_err(|error| format!("invalid gateway URL: {error}"))?;
    if url.scheme() != "https" {
        return Err("gateway must use https://".to_owned());
    }
    if url.host_str().is_none() {
        return Err("gateway URL has no host".to_owned());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("credentials must not be embedded in the gateway URL".to_owned());
    }
    if request.username.trim().is_empty() {
        return Err("username is required".to_owned());
    }
    if request.username.contains(['\n', '\r']) || request.auth_group.contains(['\n', '\r']) {
        return Err("username and group must be one line".to_owned());
    }
    if request.password.is_empty() {
        return Err("password is required".to_owned());
    }
    if request.interface.is_empty()
        || request.interface.len() > 15
        || !request
            .interface
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("interface must be 1-15 ASCII letters, digits, '-' or '_'".to_owned());
    }
    Ok(())
}

pub fn command_exists(name: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|directory| is_executable(directory.join(name)))
}

fn is_executable(path: PathBuf) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

pub fn sudo_is_ready() -> bool {
    Command::new("sudo")
        .args(["-n", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn authorize_sudo() -> Result<(), String> {
    let status = Command::new("sudo")
        .arg("-v")
        .status()
        .map_err(|error| format!("could not run sudo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("sudo authorization exited with {status}"))
    }
}

pub fn disconnect_interface(interface: &str) -> Result<usize, String> {
    let pids = openconnect_pids(interface);
    for pid in &pids {
        let status = Command::new("sudo")
            .args(["-n", "kill", "-INT", &pid.to_string()])
            .status()
            .map_err(|error| format!("could not stop openconnect PID {pid}: {error}"))?;
        if !status.success() {
            return Err(format!("could not stop openconnect PID {pid}: {status}"));
        }
    }
    Ok(pids.len())
}

pub fn openconnect_pids(interface: &str) -> Vec<u32> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut pids = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
        .filter(|pid| {
            fs::read(format!("/proc/{pid}/cmdline"))
                .map(|cmdline| cmdline_uses_interface(&cmdline, interface))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    pids.sort_unstable();
    pids
}

fn cmdline_uses_interface(cmdline: &[u8], interface: &str) -> bool {
    let arguments = cmdline
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .map(String::from_utf8_lossy)
        .collect::<Vec<_>>();
    let Some(executable) = arguments.first() else {
        return false;
    };
    if Path::new(executable.as_ref())
        .file_name()
        .and_then(|name| name.to_str())
        != Some("openconnect")
    {
        return false;
    }
    let long_option = format!("--interface={interface}");
    arguments.iter().enumerate().any(|(index, argument)| {
        argument == &long_option
            || ((argument == "--interface" || argument == "-i")
                && arguments
                    .get(index + 1)
                    .is_some_and(|value| value == interface))
    })
}

pub fn diagnostics(gateway: &str) -> Vec<(bool, String)> {
    let mut results = vec![
        (
            command_exists("openconnect"),
            "openconnect executable".to_owned(),
        ),
        (command_exists("sudo"), "sudo executable".to_owned()),
        (
            sudo_is_ready(),
            "sudo authorization cached (prompt appears on connect if needed)".to_owned(),
        ),
        (
            Path::new("/dev/net/tun").exists(),
            "/dev/net/tun available".to_owned(),
        ),
    ];

    let host = Url::parse(gateway)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned));
    let resolves = host
        .as_deref()
        .and_then(|host| (host, 443).to_socket_addrs().ok())
        .and_then(|mut addresses| addresses.next())
        .is_some();
    results.push((resolves, "gateway resolves in DNS".to_owned()));
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> ConnectionRequest {
        ConnectionRequest {
            gateway: "https://vpn.iitr.ac.in".to_owned(),
            username: "student".to_owned(),
            auth_group: String::new(),
            interface: "iitr-vpn0".to_owned(),
            password: "not-a-real-secret".to_owned(),
            second_factor: String::new(),
        }
    }

    #[test]
    fn accepts_valid_request() {
        assert!(validate_request(&valid_request()).is_ok());
    }

    #[test]
    fn rejects_non_tls_gateway() {
        let mut request = valid_request();
        request.gateway = "http://vpn.iitr.ac.in".to_owned();
        assert_eq!(
            validate_request(&request).unwrap_err(),
            "gateway must use https://"
        );
    }

    #[test]
    fn rejects_credentials_in_url() {
        let mut request = valid_request();
        request.gateway = "https://student:secret@vpn.iitr.ac.in".to_owned();
        assert!(validate_request(&request).is_err());
    }

    #[test]
    fn validates_linux_interface_name() {
        let mut request = valid_request();
        request.interface = "this-name-is-far-too-long".to_owned();
        assert!(validate_request(&request).is_err());
    }

    #[test]
    fn secrets_never_appear_in_process_arguments() {
        let mut request = valid_request();
        request.password = "distinct-password".to_owned();
        request.second_factor = "123456".to_owned();
        let arguments = openconnect_args(&request);
        let rendered = arguments
            .iter()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!rendered.contains("distinct-password"));
        assert!(!rendered.contains("123456"));
        assert!(rendered.contains("--passwd-on-stdin"));
    }

    #[test]
    fn identifies_only_openconnect_for_our_interface() {
        let matching = b"/usr/bin/openconnect\0--protocol=anyconnect\0--interface=iitr-vpn0\0";
        let other_interface = b"openconnect\0-i\0work-vpn0\0";
        let sudo_wrapper = b"sudo\0openconnect\0--interface=iitr-vpn0\0";
        assert!(cmdline_uses_interface(matching, "iitr-vpn0"));
        assert!(!cmdline_uses_interface(other_interface, "iitr-vpn0"));
        assert!(!cmdline_uses_interface(sudo_wrapper, "iitr-vpn0"));
    }
}
