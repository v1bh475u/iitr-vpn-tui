# Possible future improvements

This is an unmaintained project; the following items are suggestions for forks,
not a roadmap or promise.

- Add interactive handling for arbitrary multi-step authentication forms rather
  than pre-supplying password and OTP lines.
- Support browser-based SAML/SSO flows when IITR enables them.
- Provide a NetworkManager backend for desktop-managed routes and DNS.
- Detect and display existing `iitr-vpn0` sessions when the TUI starts.
- Show negotiated protocol, server address, assigned address, routes, uptime,
  and byte counters in a dedicated statistics view.
- Offer opt-in, redacted persistent logs with strict file permissions.
- Test terminal restoration under signals, panics, SSH disconnects, and terminal
  resize events.
- Package maintained forks for the AUR and other Linux distributions.
- Add accessibility-focused color themes and a non-interactive status command.
- Review secret memory handling with platform-specific locked memory where that
  meaningfully improves the threat model.

