# Security policy

## Support status

There are no supported versions and no security-response commitment because
this project is unmaintained. Do not depend on it for a critical or managed
environment without performing your own review.

## Sensitive information

Do not publish IITR passwords, OTPs, VPN session cookies, private hostnames,
assigned addresses, or unredacted diagnostic logs. Revoke or rotate anything
accidentally disclosed.

The application is designed to:

- pass passwords and second factors through OpenConnect's stdin;
- exclude secrets from argv, environment variables, configuration, and logs;
- store non-secret configuration with mode `0600`;
- validate HTTPS gateway URLs;
- invoke commands without a shell;
- elevate only OpenConnect and its disconnect signal.

These properties reduce exposure but do not constitute a formal security audit.
OpenConnect, the operating system, the IITR gateway, and local sudo policy remain
part of the trusted computing base.

## Reporting

Suggestions or vulnerability reports may be opened without sensitive details,
but they may not receive a response. For an actionable or time-sensitive issue,
maintain a patched fork and coordinate with IITR or the relevant upstream
project as appropriate.

