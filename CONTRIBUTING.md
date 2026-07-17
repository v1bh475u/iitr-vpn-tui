# Contributing

Thank you for considering an improvement. This repository is intentionally
unmaintained, so issues and pull requests may remain unanswered indefinitely.
A fork is recommended for changes you need to rely on.

Suggestions are still welcome, particularly when they include reproducible
details and avoid account data. Useful reports contain:

- distribution and kernel version;
- `openconnect --version` output;
- the exact non-secret error message;
- whether password-only, OTP, hardware-token, or browser SSO authentication is
  in use;
- redacted routes and DNS behavior after connection.

Never include passwords, OTPs, VPN cookies, complete logs containing personal
data, or private IITR resources.

If you prepare a change in a fork, run:

```sh
cargo fmt -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
```

Prefer small conventional commits such as `feat:`, `fix:`, `docs:`, `test:`,
`refactor:`, `ci:`, or `chore:`. Sign commits and include a DCO sign-off when
possible:

```sh
git commit -s -S -m "fix: describe the focused change"
```

