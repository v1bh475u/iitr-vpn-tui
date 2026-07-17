# IITR VPN TUI

> [!NOTE]
> This is an entirely vibe-coded project. Its source, tests, and documentation
> were produced through AI-assisted conversational development and should be
> reviewed accordingly.


> [!IMPORTANT]
> This is an unofficial, personal project. It is not affiliated with or
> supported by IIT Roorkee. **I do not intend to maintain this repository or
> provide user support.** It is published as-is so others can study, use, or
> fork it. Suggestions and improvements are welcome, but may not receive a
> response; a maintained fork is the best path forward.

If this project helped you, consider starring the repository as a small gesture
of appreciation.

`iitr-vpn` is a small Rust terminal client for IIT Roorkee's Cisco AnyConnect
gateway. It replaces the obsolete Debian-oriented installer in this directory
with a distro-friendly UI backed by the maintained, open-source `openconnect`
client.

The gateway defaults to `https://vpn.iitr.ac.in`. The public endpoint was
verified on 17 July 2026: it presents a valid `*.iitr.ac.in` certificate and
Cisco `+CSCOE+` endpoints.

## Install on Arch Linux

Install the runtime and build dependencies:

```sh
yay -S --needed openconnect rust
```

The packages are in Arch's official repositories; `yay` delegates their
installation to pacman and will ask for your sudo password locally.

Build and install for your user:

```sh
cargo build --release
install -Dm755 target/release/iitr-vpn ~/.local/bin/iitr-vpn
```

Ensure `~/.local/bin` is in `PATH`, then run:

```sh
iitr-vpn
```

Check the installed command or print its version with:

```sh
iitr-vpn --help
iitr-vpn --version
```

Do not run the TUI itself as root. It asks `sudo` for authorization only when
connecting, then runs only `openconnect` and the disconnect signal with elevated
permissions.

## Use

1. Enter your IITR username. The live gateway's `IITR-RA-VPN` auth group is
   prefilled; change it only if IITR gives you a different group name.
2. Enter your password and, if applicable, the current OTP/second factor.
3. Press `Enter` from either secret field, or press `c` while a secret field is
   selected. Approve the normal `sudo` prompt.
4. Press `d` to disconnect. If sudo's cached authorization has expired, the TUI
   temporarily restores the normal terminal so you can enter your sudo password,
   then returns and stops the tunnel. The app deliberately refuses to quit while
   its VPN process is active.

Keys:

- `Tab` / `Shift-Tab`: move between fields
- `c`: connect (from a secret field)
- `d`: disconnect
- `r`: check `openconnect`, `sudo`, TUN, and gateway DNS
- `s`: save non-secret settings
- `Esc`: erase both secret fields
- `q` or `Ctrl-C`: quit when disconnected

If the terminal or TUI crashes while OpenConnect remains active, recover from a
normal terminal with:

```sh
iitr-vpn --disconnect
```

This finds only OpenConnect processes using the dedicated `iitr-vpn0` interface,
prompts for sudo normally, and sends a clean interrupt.

Text shortcuts are active while a secret field is selected so typing a `c`,
`d`, or `q` in a username or URL still works normally.

The configuration is stored at
`$XDG_CONFIG_HOME/iitr-vpn/config.toml` (normally
`~/.config/iitr-vpn/config.toml`) with mode `0600`. It contains only the gateway,
username, optional auth group, and interface name. Passwords and OTPs are never
saved, placed in process arguments, or placed in environment variables.

## Why `openconnect`?

The supplied archive is Cisco AnyConnect 4.7.04056 from 2019. It does not
contain an IITR VPN profile or any IITR-specific networking code. `openconnect`
implements the same Cisco AnyConnect protocol, is packaged by Arch, follows
current OpenSSL/kernel changes, and does not require a permanently running
proprietary agent under `/opt`.

See [docs/REVERSE_ENGINEERING.md](docs/REVERSE_ENGINEERING.md) for the full
archive analysis and design mapping.

## Limits of local verification

The build, command construction, configuration permissions, and UI can be
tested without credentials. A complete tunnel cannot be established without a
valid IITR account and second factor. If authentication fails, check the session
log and set the auth group IITR provides; do not disable certificate validation.

## Documentation

- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Reverse-engineering notes](docs/REVERSE_ENGINEERING.md)
- [Possible future improvements](docs/IDEAS.md)
- [Contribution policy](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Maintenance status](MAINTENANCE.md)

## License

The Rust implementation and its documentation are available under the MIT
license. Cisco's original installer and binaries are not part of this Git
project and remain subject to Cisco's own terms.
