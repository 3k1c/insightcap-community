# Installation

InsightCAP Community Edition installers are distributed through GitHub Releases.
Installer files and binary artifacts should not be committed directly to Git
history.

## Download

Use the latest release from:

https://github.com/3k1c/insightcap-community/releases

Each release should include release notes that describe:

- Version number.
- Supported platform.
- Installer file name.
- Known limitations, if any.
- License scope.
- Verification notes, if available.

## Usage Rights

InsightCAP Community Edition binaries are free to use for personal and
commercial purposes under the Community Edition License.

Commercial Edition features, proprietary modules, hosted services, enterprise
licensing, and private source code are not included in the Community Edition
License.

See:

- `COMMUNITY-LICENSE.md`
- `docs/open-source-scope.md`

## Installation Steps

1. Open the latest GitHub Release.
2. Download the installer for your platform.
3. Review the release notes before installing.
4. Run the installer.
5. Start InsightCAP Community from the installed application entry point.

If a release provides checksums or verification notes, compare the downloaded
installer against the published release information before installation.

## Release Artifact Policy

Installers and binary packages should be attached to GitHub Releases.

They should not be committed to this repository as regular files because:

- Binary files make Git history large.
- Release assets need version-specific context.
- GitHub Releases provide a clearer download and changelog workflow.
- Public repositories should keep source, documentation, and release metadata
  separate from binary artifacts.

The repository `.gitignore` intentionally excludes common installer and archive
formats.

## Public Release Checklist

Before publishing an installer, review the release artifact for:

- Personal email addresses.
- Local user paths.
- Device or machine names.
- Operating system build details from development machines.
- Debug-only settings.
- Local databases.
- Local cache files.
- API keys, tokens, credentials, or secrets.
- Internal-only documentation.

## Source Code

The public repository may include selected community-facing source code.

The installer may include additional application components that are not fully
published in this repository. Those components remain outside the open source
scope unless explicitly included here under a stated license.

## Support Scope

The Community Edition is provided as a public community release. Future
Commercial Edition features, enterprise support, hosted services, or proprietary
modules may be offered separately.
