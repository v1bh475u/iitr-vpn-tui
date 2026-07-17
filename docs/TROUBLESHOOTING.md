# Troubleshooting

## Run the built-in checks

Focus either secret field and press `r`. The log pane checks whether
`openconnect` and `sudo` exist, `/dev/net/tun` is available, and the gateway
resolves in DNS.

The equivalent basic shell checks are:

```sh
openconnect --version
test -c /dev/net/tun && echo "TUN ready"
getent ahosts vpn.iitr.ac.in
```

## The sudo password cannot be entered

Current versions leave raw/alternate-screen mode before both connect and
disconnect authorization, so the normal sudo prompt accepts input. If an older
TUI or crashed terminal leaves a tunnel behind, open a normal terminal and run:

```sh
iitr-vpn --disconnect
```

Do not kill OpenConnect with `SIGKILL`: a clean interrupt gives its routing and
DNS script a chance to undo system changes.

## Authentication fails

Confirm all of the following:

- gateway is `https://vpn.iitr.ac.in`;
- auth group is `IITR-RA-VPN`, unless IITR supplied another value;
- username has no domain prefix or suffix unless IITR requires one;
- the OTP is current and entered only when the account requires it;
- system time is synchronized, because TOTP depends on accurate time.

Repeated login attempts may trigger institutional rate limits. Do not disable
certificate verification to work around an authentication error.

## Connected but IITR names do not resolve

Inspect the interface and resolver state:

```sh
ip -brief address show dev iitr-vpn0
ip route show table all dev iitr-vpn0
resolvectl status iitr-vpn0
```

An address and routes prove that the tunnel is up, but the gateway may not push
DNS servers. Avoid hard-coding guessed institutional DNS addresses. Use values
provided by IITR and remove local overrides after disconnecting.

## The interface already exists

Recover any orphaned session first:

```sh
iitr-vpn --disconnect
```

Wait a few seconds and verify that `ip link show iitr-vpn0` reports no device
before reconnecting.

## Collecting useful diagnostics

The TUI currently keeps a bounded session log in memory and does not persist it.
Copy only the relevant error and redact usernames, addresses, cookies, internal
hostnames, and other account or institutional data before sharing it.

