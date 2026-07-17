# Reverse-engineering notes

## What the supplied bundle does

The root `Install.run` is only a five-line shell wrapper. It:

1. runs `apt-get install libcanberra-gtk-module`;
2. executes Cisco's `vpn_install.sh` as root;
3. executes Cisco's `dart_install.sh` as root.

That is the Debian-only assumption. The GTK package supports Cisco's old GUI;
it is not part of the VPN protocol. DART is Cisco's Diagnostic AnyConnect
Reporting Tool and is not needed to create a tunnel.

`VPN.zip` and the already-extracted directory are the same generic Cisco
AnyConnect 4.7.04056 Linux distribution from June 2019. The important pieces
are:

- `vpn`: Cisco's CLI, which talks to the agent rather than implementing a
  self-contained IITR workflow;
- `vpnui`: the GTK UI;
- `vpnagentd`: a privileged, persistent tunnel agent;
- `vpn_install.sh`: copies binaries and bundled libraries into
  `/opt/cisco/anyconnect`, installs a systemd/SysV service, and loads TUN;
- `dart/`: a large diagnostics collector;
- `posture/` and `nvm/`: optional posture and network-visibility modules that
  the root wrapper does not install.

The ELF files are x86-64 glibc binaries and carry an RPATH of
`/opt/cisco/anyconnect/lib`. Cisco bundled its own SSL, crypto, curl, Boost, and
VPN libraries. This explains why copying only `vpn` onto Arch is insufficient.
The service itself is conventional: load `tun`, then start `vpnagentd`.

No IITR hostname, profile, auth group, certificate pin, username format, route,
or DNS rule appears in the archive. The only site configuration therefore has
to come from the gateway after login. Installing all 58 MB does not add IITR
knowledge.

## Protocol identification

The bundle identifies itself as Cisco AnyConnect and its CLI accepts a VPN host.
The live `vpn.iitr.ac.in` HTTPS service was checked on 17 July 2026. It presented
a publicly trusted `*.iitr.ac.in` certificate and responded with Cisco's
`/+CSCOE+/...` endpoint family. Those endpoints identify the Cisco
SSL/AnyConnect protocol supported by `openconnect --protocol=anyconnect`.
A credential-free OpenConnect XML handshake also advertised the auth group
`IITR-RA-VPN`; the TUI uses it as the default without hard-coding any account
credentials.

No certificate fingerprint is pinned in the application: the gateway currently
uses a normal public certificate, and hard-coding its leaf fingerprint would
break legitimate renewals. `openconnect` performs standard hostname and trust
chain verification.

## Old-to-new mapping

| Original component | Rust tool equivalent |
| --- | --- |
| GTK `vpnui` | Ratatui/crossterm terminal UI |
| Cisco `vpn` + `vpnagentd` | Arch `openconnect` process |
| bundled Cisco protocol libraries | maintained system OpenConnect/OpenSSL stack |
| root installation under `/opt` | unprivileged binary under `~/.local/bin` |
| always-on privileged systemd agent | `sudo openconnect` only for one session |
| `/sbin/modprobe tun` script | kernel `/dev/net/tun` check; modern Arch provides TUN |
| Cisco preferences/profile files | minimal XDG TOML file, no secrets |
| DART diagnostics package | dependency/TUN/DNS checks plus bounded session log |

## Process and secret model

The TUI validates an HTTPS gateway and Linux interface name, then invokes a
fixed executable/argument vector; no shell interprets user input. It starts:

```text
sudo -n openconnect --protocol=anyconnect --passwd-on-stdin ... GATEWAY
```

The password and optional second factor go through the child's stdin. They are
not command-line arguments (visible in `ps`), environment variables, config, or
logs. Secret buffers are zeroed when handed off, cleared, or dropped.

The connection runs in its own process group. On disconnect, the tool searches
`/proc` for the actual OpenConnect executable with the exact dedicated
`--interface=iitr-vpn0` argument, then sends that root process SIGINT so
OpenConnect restores routes and DNS cleanly. This remains available outside the
TUI as `iitr-vpn --disconnect`, allowing recovery after a terminal or TUI crash.

## Remaining live test boundary

TLS reachability and protocol identity can be checked publicly. Authentication,
the assigned IP, IITR routes, campus DNS, DTLS/ESP negotiation, and any account-
specific auth group require an IITR account. The tool intentionally does not
try to bypass those controls or weaken TLS when a login fails.
