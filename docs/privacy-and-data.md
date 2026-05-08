# Privacy and Data

This document describes the public privacy and data-handling expectations for
InsightCAP Community. It is intended for users, reviewers, and contributors.

InsightCAP Community is documented as a local-first desktop application. The
public repository should not contain private user data, local development data,
secrets, or machine-specific metadata.

## Local-First Direction

InsightCAP Community is designed around a local-first product direction. This
means the application is intended to prioritize local workflows and user control
over personal knowledge data where applicable.

Public documentation should describe this direction clearly without exposing
private implementation details, proprietary modules, or internal policies.

## Repository Data Boundary

This repository is for public materials only. It may contain:

- Public documentation.
- Community Edition license information.
- Installation notes.
- High-level architecture notes.
- Selected non-sensitive source code, if added later.
- Public release notes.

This repository must not contain:

- User databases.
- Local cache files.
- Embedding caches.
- Environment files such as `.env`.
- API keys, tokens, credentials, or secrets.
- Private emails.
- Local user paths.
- Local Windows usernames.
- Device names.
- Operating system build details from development machines.
- Internal-only documentation.
- Proprietary implementation details.

## Release Artifacts

Installers and binary artifacts should be distributed through GitHub Releases,
not committed directly to Git history.

Before publishing an installer, release assets should be reviewed for:

- Unexpected embedded metadata.
- Local file paths.
- Developer machine names.
- Private email addresses.
- Debug-only configuration.
- Bundled local databases or cache files.
- Credentials or API keys.

## Development Metadata

Public commits should use the public GitHub identity:

```text
3k1c <169649816+3k1c@users.noreply.github.com>
```

The public repository should not use private email addresses or local developer
identity metadata in commit history.

## Public Readiness Checklist

Before making this repository public, run a review that checks:

1. No personal email addresses are present.
2. No local Windows user paths are present.
3. No machine names are present.
4. No operating system build details from development machines are present.
5. No `.env` files, tokens, credentials, or API keys are present.
6. No user databases or local cache files are present.
7. No private roadmap, internal policy, or proprietary implementation detail is
   included.
8. Git history uses the public GitHub noreply identity.
9. Installers are attached to GitHub Releases rather than committed to the repo.
10. License scope is clearly stated in `LICENSE`, `COMMUNITY-LICENSE.md`, and
    `docs/open-source-scope.md`.

## User Data

This repository does not include user data.

If selected source code is added later, it should be reviewed to ensure it does
not contain test fixtures, local exports, screenshots, logs, or sample data that
could identify a real person, device, account, or organization.

## Commercial Edition Boundary

Future Commercial Edition features, proprietary modules, hosted services,
enterprise licensing, and private source code are outside the scope of this
community repository.

Any future commercial or hosted service privacy terms should be documented
separately from this Community Edition repository.
