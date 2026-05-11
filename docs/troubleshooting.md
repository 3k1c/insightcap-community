# Troubleshooting

This guide lists common InsightCAP Community beta issues and practical checks.

InsightCAP Community v0.9.0-beta.1 is a beta release. Behavior may depend on the
local Windows environment, available services, network access, configured AI
settings, and third party platform behavior.

## Installation

### Installer cannot start

Check that:

- The installer was downloaded from the GitHub Release assets.
- The file name matches the release notes.
- Windows security prompts were reviewed.
- The installer file is not blocked by browser or antivirus policy.
- The downloaded file checksum matches the published SHA256 value, if provided.

If the checksum does not match, delete the installer and download it again.

### Application does not launch after installation

Try these checks:

- Start InsightCAP from the installed application entry point.
- Restart Windows and try again.
- Confirm that the installation completed without errors.
- Check whether antivirus or endpoint protection blocked the application.
- Reinstall the same release if the installation appears incomplete.

## First-time Setup

### Recovery phrase was not saved

The 24-word recovery phrase is required if the local decryption key becomes
unavailable.

If the first-time setup has not been completed, restart setup and save the
recovery phrase securely.

If setup has already completed and the recovery phrase was not saved, the user
should avoid clearing the local decryption key or deleting recovery-related
material.

### Knowledge folder cannot be selected

Check that:

- The selected folder is accessible by the current Windows user account.
- The folder is not read-only.
- The folder path is not on a disconnected drive.
- The folder is not blocked by security software.
- The folder has enough disk space.

## Capture and Extraction

### Web page capture is incomplete

Web capture is best-effort.

Incomplete extraction may happen when the page uses:

- Heavy client-side rendering.
- Login-only content.
- Infinite scrolling.
- Embedded apps.
- Anti-bot protection.
- Paywalls.
- Content loaded only after user interaction.

Try opening the page normally, copying the important text manually, or using a
simpler source page if available.

### Document extraction is incomplete

Document extraction depends on file structure.

Limited results may happen with:

- Scanned documents.
- Image-only PDFs.
- Protected or encrypted files.
- Complex layouts.
- Unusual encodings.
- Text embedded as images.

Review extracted text before saving or using it.

### Video URL capture fails

Video URL capture may depend on subtitles, transcripts, metadata, network
access, and platform behavior.

Check that:

- The video is publicly accessible.
- Subtitles or transcript data are available.
- The network connection is working.
- The platform has not changed access behavior.
- Local helper tools, if required by the installed release, are available.

## AI-assisted Workflows

### AI features do not respond

Check that:

- AI service settings are configured.
- The selected model is available.
- The network connection is working if the configured service requires network
  access.
- The service endpoint is reachable.
- Any required service credentials are entered correctly.

Do not commit service credentials, tokens, or API keys to this repository.

### AI output quality is poor

AI output depends on the configured model, prompt, source content, and available
context.

Try:

- Reviewing the captured source text first.
- Shortening very long input.
- Splitting unrelated material into separate tasks.
- Rewriting the instruction more clearly.
- Trying a different configured model if available.

## Telegram Bot Workflows

### Telegram Bot does not respond

Check that:

- The bot token is configured correctly.
- The current user is allowed to use the bot.
- Network access is available.
- Telegram service access is not blocked.
- The message type is supported by the configured workflow.

Telegram platform limits may affect message size and response behavior.

## Export

### Exported file formatting is different from the editor

Export results may vary by target format.

Check that:

- The document was reviewed before export.
- The selected export format supports the expected formatting.
- The exported file was opened with a compatible application.
- Very complex layouts were simplified before export.

## Uninstall and Reinstall

### Existing knowledge base cannot be unlocked after reinstall

If the local decryption key was cleared or is unavailable, the 24-word recovery
phrase is required.

If both the local decryption key and the recovery phrase are unavailable, the
encrypted knowledge base cannot be recovered.

### User data was not deleted during uninstall

This is expected by default.

InsightCAP uninstall should remove program files without deleting user data
unless the user explicitly selects destructive uninstall options.

Optional uninstall choices may include:

- Delete local app data.
- Clear the current Windows account's local decryption key.
- Permanently delete the selected custom knowledge folder.

## Reporting Issues

When reporting an issue, include:

- InsightCAP Community version.
- Windows version.
- A short description of the problem.
- Steps to reproduce the issue.
- Whether the issue happens consistently.
- Relevant error messages.

Do not include:

- Recovery phrases.
- API keys, tokens, credentials, or secrets.
- Personal databases.
- Private documents.
- Local user paths if they reveal personal information.
