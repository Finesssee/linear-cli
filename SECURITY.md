# Security

`linear-cli` is a local-first CLI for Linear.app. Its primary security concerns are credential handling, safe interaction with untrusted Linear and GitHub responses, safe subprocess execution, and cautious handling of local listeners, exported data, and downloaded uploads.

## Supported versions

| Version | Supported |
| --- | --- |
| `0.3.22` | Yes |
| `0.3.21` and earlier | No |

Security fixes and documentation updates are only guaranteed on the latest release line.

## Reporting a vulnerability

Please avoid posting sensitive exploit details in a public GitHub issue.

Preferred reporting path:

1. Use GitHub's private vulnerability reporting for this repository if it is available.
2. If private reporting is unavailable, contact the maintainer through GitHub first and share only the minimum details needed to reproduce the issue privately.

Include:

- the `linear-cli` version
- your OS and installation path
- whether you used API-key auth or OAuth
- whether the issue requires local access, a malicious Linear workspace member, or network access
- a minimal reproduction, logs, or screenshots with secrets redacted

## Security notes

- API keys and OAuth tokens may come from `LINEAR_API_KEY`, OS keyring storage, or the config file under the user's config directory. Direct API-key entry on the command line is intentionally avoided for persistent config flows.
- The OAuth callback listener binds to `127.0.0.1` and validates both `state` and PKCE before exchanging the authorization code.
- The optional webhook listener defaults to `127.0.0.1`, verifies `linear-signature` with HMAC-SHA256, and enforces request size and timeout limits.
- Upload downloads are restricted to `https://uploads.linear.app` and only follow redirects that stay on that host.
- The update flow checks GitHub Releases and shells out to local Cargo tooling rather than executing shell text. Installation only runs through the explicit `linear-cli update` path.

## Additional documentation

- Detailed threat model: [docs/security-threat-model.md](docs/security-threat-model.md)
- Installation and usage: [README.md](README.md)
