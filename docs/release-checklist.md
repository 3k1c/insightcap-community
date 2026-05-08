# Release Checklist

This checklist should be used before making this repository public and before
publishing each InsightCAP Community Edition release.

It is intended to keep the public repository clean, intentional, and free of
private development information.

## Repository Visibility

- [ ] Confirm the repository is still private during preparation.
- [ ] Confirm all public documentation has been reviewed.
- [ ] Confirm the repository is intended to become public only after this
      checklist is complete.
- [ ] Confirm no private repository history has been imported.

## Git Identity and History

- [ ] Confirm commits use the public GitHub identity:

      ```text
      3k1c <169649816+3k1c@users.noreply.github.com>
      ```

- [ ] Confirm Git history does not contain private email addresses.
- [ ] Confirm Git history does not contain local machine names.
- [ ] Confirm Git history does not contain local user paths.
- [ ] Confirm Git history does not contain secrets or credentials.

## Personal Information Review

- [ ] Search for personal email addresses.
- [ ] Search for local user paths.
- [ ] Search for device or machine names.
- [ ] Search for operating system build details from development machines.
- [ ] Search for screenshots, logs, or fixtures that may identify a person,
      device, account, or organization.

## Secrets and Credentials

- [ ] Confirm no `.env` files are committed.
- [ ] Confirm no API keys are committed.
- [ ] Confirm no tokens are committed.
- [ ] Confirm no credentials are committed.
- [ ] Confirm no signing keys or certificates are committed.

## Local Data and Generated Files

- [ ] Confirm no user databases are committed.
- [ ] Confirm no local cache files are committed.
- [ ] Confirm no embedding cache files are committed.
- [ ] Confirm no generated vector stores are committed.
- [ ] Confirm no build output directories are committed.
- [ ] Confirm no installer files are committed directly to Git history.

## Documentation Review

- [ ] Confirm `README.md` describes the repository accurately.
- [ ] Confirm `docs/architecture.md` stays at a public architecture level.
- [ ] Confirm `docs/privacy-and-data.md` describes data boundaries clearly.
- [ ] Confirm `docs/open-source-scope.md` clearly defines what is and is not
      included.
- [ ] Confirm `docs/installation.md` points users to GitHub Releases.
- [ ] Confirm documentation does not expose internal prompts, policies, private
      roadmap items, or proprietary implementation details.

## License Review

- [ ] Confirm `LICENSE` is present.
- [ ] Confirm `COMMUNITY-LICENSE.md` is present.
- [ ] Confirm the MIT License applies only to source code and documentation
      included in this repository unless otherwise stated.
- [ ] Confirm the Community Edition License applies to Community Edition
      binaries and public release materials.
- [ ] Confirm Commercial Edition features, proprietary modules, hosted services,
      enterprise licensing, and private source code are excluded.

## Release Artifact Review

- [ ] Confirm installers are uploaded as GitHub Release assets.
- [ ] Confirm installers are not committed to Git history.
- [ ] Confirm release notes include the version number.
- [ ] Confirm release notes describe the supported platform.
- [ ] Confirm release notes describe the license scope.
- [ ] Confirm checksums or verification notes are included if available.
- [ ] Confirm release assets do not contain debug-only configuration.
- [ ] Confirm release assets do not contain local databases, caches, secrets, or
      development metadata.

## Final Public Release Gate

- [ ] Run a repository-wide sensitive information scan.
- [ ] Review all open files and staged changes before publishing.
- [ ] Confirm the repository still represents only the intended public subset of
      InsightCAP.
- [ ] Confirm the public/private boundary is clear to users and contributors.
- [ ] After all checks pass, change the repository visibility to public.
