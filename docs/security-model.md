# Security Model

This document describes the public, high-level security model for InsightCAP
Community.

It is intentionally limited to user-facing concepts. It does not describe
private implementation details, cryptographic internals, or proprietary source
code.

## Local-first Security Direction

InsightCAP Community is designed as a local-first desktop application.

The security model is built around these ideas:

- Personal knowledge data should remain under user control.
- Local knowledge data should be encrypted.
- Recovery material should be handled carefully by the user.
- Destructive uninstall choices should be explicit and opt-in.
- Public documentation should explain user impact without exposing private
  implementation details.

## Encrypted Local Knowledge Base

InsightCAP stores personal knowledge in an encrypted local knowledge base.

Encryption is intended to protect knowledge data at rest. Users should still
protect their Windows account, device, backups, and recovery phrase.

The encrypted knowledge base should not be treated as recoverable unless the
required decryption material is available.

## Local Decryption Key

InsightCAP keeps a local decryption key for the current Windows user account.

This key is separate from the encrypted knowledge data.

If the local decryption key remains available, the existing encrypted knowledge
base can be unlocked by the installed application.

If the local decryption key is removed, lost, or unavailable, the user must use
the 24-word recovery phrase saved during first-time setup.

## 24-word Recovery Phrase

During first-time setup, users should save the 24-word recovery phrase in a
secure place.

The recovery phrase is required when the encrypted knowledge base must be
unlocked without the local decryption key.

Users should not store the recovery phrase in the same location as the encrypted
knowledge base unless they understand the risk.

If both the local decryption key and the recovery phrase are unavailable, the
encrypted knowledge base cannot be recovered.

## App Data and Knowledge Data Separation

InsightCAP separates these categories:

- Application program files.
- Local app data, such as settings, cache files, downloaded models, and the
  default knowledge base.
- User-selected custom knowledge folders.
- Local decryption key material for the current Windows user account.

A normal uninstall should remove the application program files only.

Local app data, decryption key material, and custom knowledge folders should
only be removed when the user explicitly selects the corresponding uninstall
option.

## Uninstall Risk

The uninstaller may offer optional destructive choices.

Users should understand the impact before selecting them:

- Deleting local app data may remove settings, caches, downloaded models, and
  the default local knowledge base.
- Clearing the local decryption key may require the 24-word recovery phrase
  after reinstalling.
- Permanently deleting a custom knowledge folder cannot be undone.

All destructive uninstall choices should be unchecked by default.

## User Responsibilities

Users should:

- Save the 24-word recovery phrase securely.
- Keep device access protected.
- Back up important knowledge data when needed.
- Review captured or AI-edited content before relying on it.
- Understand the effect of uninstall options before selecting destructive
  choices.

## Public Repository Boundary

This repository should not contain:

- User databases.
- Local cache files.
- Recovery phrases.
- Local decryption keys.
- API keys, tokens, credentials, or secrets.
- Private implementation details.
- Internal-only documentation.
- Machine-specific metadata.

Installers and binary artifacts should be distributed through GitHub Releases,
not committed directly to Git history.
