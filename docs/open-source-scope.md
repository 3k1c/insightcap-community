# Open Source Scope

This document defines the public open source scope for InsightCAP Community.
It clarifies what is included in this repository, what is intentionally not
included, and how the Community Edition relates to future Commercial Edition
features.

## Repository Purpose

`insightcap-community` is the public community repository for InsightCAP.

The repository is intended to provide:

- A transparent public entry point for InsightCAP Community.
- Public documentation for installation, architecture, privacy, and licensing.
- Selected community-facing source code, if added later.
- Release notes and links to Community Edition installers.
- A clear boundary between public materials and proprietary implementation.

This repository is not intended to be a full mirror of the private InsightCAP
development repository.

## Included

This repository may include:

- Public README and product overview.
- Community Edition license information.
- MIT-licensed source code and documentation.
- High-level architecture notes.
- Installation notes.
- Privacy and local-data notes.
- Public release notes.
- Issue templates and contribution guidelines.
- Selected non-sensitive source modules.
- Example workflows or simplified reference implementations.

Any source code added to this repository should be reviewed before publication
to confirm that it does not expose private implementation details, credentials,
personal information, local development metadata, or proprietary business logic.

## Not Included

This repository does not include:

- Private source code.
- Proprietary modules.
- Commercial Edition features.
- Hosted services.
- Enterprise licensing materials.
- Internal prompts or policies.
- Private or unreleased integration features, synchronization logic, indexing
  logic, retrieval logic, schemas, and related source code.
- Internal ranking, retrieval, processing, or product strategy details that are
  not intended for public release.
- Private roadmap or planning documents.
- Internal debugging notes.
- User data.
- Local databases.
- Local caches.
- Embedding caches.
- Build artifacts.
- Installers committed directly to Git history.
- API keys, tokens, credentials, or secrets.
- Personal email addresses.
- Local user paths, device names, or operating system build details from
  development machines.

## Source Code License

Unless otherwise stated, source code and documentation in this repository are
licensed under the MIT License.

The MIT License applies only to materials that are actually included in this
repository. It does not apply to private source code, proprietary modules,
Commercial Edition features, hosted services, or internal implementation
details that are not published here.

## Community Edition Binary License

InsightCAP Community Edition binaries are free to use for personal and
commercial purposes under the Community Edition License.

The Community Edition License applies only to Community Edition binaries,
documentation, and public materials distributed through this repository or its
GitHub Releases.

It does not grant rights to:

- Commercial Edition features.
- Proprietary modules.
- Hosted services.
- Enterprise features.
- Private source code.
- Trademarks or branding beyond normal reference to InsightCAP Community.

## Commercial Edition Boundary

3k1c may offer Commercial Edition features, paid services, proprietary modules,
enterprise licensing, or hosted services separately in the future.

Those future offerings are outside the scope of this repository unless they are
explicitly added here under a stated license.

Community Edition availability does not imply that all future InsightCAP
features will be open source or free of charge.

## Contributions

If this repository later accepts community contributions, contributions should
be limited to the public scope of this repository.

Examples of acceptable contribution areas may include:

- Documentation improvements.
- Installation notes.
- Public examples.
- Non-sensitive UI or utility code that has been explicitly included.
- Issue reports for Community Edition releases.

Contributions should not include:

- Private implementation details.
- Secrets or credentials.
- User data.
- Proprietary code copied from private repositories.
- Decompiled or reverse-engineered Commercial Edition code.
- Local machine metadata or personal information.

## Release Artifacts

Installers and binary packages should be attached to GitHub Releases rather
than committed to Git history.

Release notes should clearly state:

- Version number.
- Supported platform.
- Installation notes.
- License scope.
- Known limitations, if any.

## Public Readiness Rule

Before this repository is made public, and before each future release, the
repository should be reviewed against the privacy and data checklist in:

- `docs/privacy-and-data.md`

The public repository should remain a clean, intentional subset of InsightCAP,
not an accidental export of the private development repository.
