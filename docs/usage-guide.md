# InsightCAP Community Usage Guide

This guide explains the main Community Edition setup and usage workflows.
InsightCAP is a local-first desktop application for turning captured content
into an encrypted personal knowledge base that can be searched, reused, edited,
and exported.

## First-time Setup

1. Download the Community Edition installer from the GitHub Release assets.
2. Run the installer and launch InsightCAP Community.
3. Complete the first-time setup flow.
4. Choose the interface language and global hotkeys.
5. Select or create the local knowledge folder.
6. Save the 24-word recovery phrase in a secure place.
7. Configure optional AI service settings if AI-assisted workflows are needed.
8. Configure optional Telegram Bot workflows if capture, reminder, or knowledge
   Q&A flows are needed outside the desktop window.

The 24-word recovery phrase is important. If the local decryption key is removed
or unavailable, the recovery phrase is required to unlock the existing encrypted
knowledge base.

## Local Data and Encryption

InsightCAP separates local app data, the user-selected knowledge folder, and the
local decryption key.

- The local knowledge base is encrypted.
- The local decryption key is kept under the current Windows user account.
- The user-selected knowledge folder stores knowledge data chosen by the user.
- Local app data may include settings, cache files, downloaded models, and the
  default knowledge base.
- A normal uninstall removes the application program files and does not delete
  user data by default.
- Custom knowledge folders are not deleted unless the user explicitly chooses to
  permanently delete them during uninstall.

If the local decryption key is cleared but the encrypted knowledge base is kept,
the recovery phrase is required after reinstalling. Without the recovery phrase,
the encrypted knowledge base cannot be recovered.

## Capture and Extraction

InsightCAP is designed to reduce manual copying and reorganizing by extracting
content from different source types.

Supported workflow categories include:

- Web pages.
- Local documents.
- Supported video URLs such as YouTube and Bilibili.
- Clipboard captures.
- Notes and structured text.
- Telegram Bot capture inputs.

After capture, extracted content can be reviewed before it becomes reusable
knowledge.

## Review and AI Text Editing

Captured content can be cleaned up, rewritten, summarized, or shaped into more
useful drafts through AI-assisted editing workflows.

Typical uses include:

- Turning raw extracted content into structured notes.
- Creating summaries from long documents or extracted video URL content.
- Preparing reusable drafts from captured research.
- Editing AI-generated text before saving or exporting it.

AI-assisted workflows may require user-provided AI service settings depending on
the configured environment.

## Projects and Conversations

InsightCAP supports project-oriented and conversation-oriented workflows.

Projects help keep related research, writing, planning, and follow-up material
together. Conversations help users reuse saved knowledge in new discussion or
drafting contexts without losing the connection to the original source material.

## Personal Memory System

The personal memory system stores captured and reviewed knowledge for later
search, recall, and reuse.

It is intended for workflows such as:

- Searching previously captured knowledge.
- Reusing knowledge across projects and conversations.
- Keeping source material connected to follow-up work.
- Building a long-term personal knowledge base.

## Telegram Bot Capture, Reminder, and Q&A Workflows

Telegram Bot workflows can support lightweight capture, reminder requests,
knowledge-base questions, and project or conversation commands outside the main
desktop window.

These workflows are useful when a user wants to capture input, ask questions, or
manage follow-up actions without opening the full desktop interface.

## AI Text Editor Export

InsightCAP can export the current AI text editor document to TXT, Markdown,
HTML, DOCX, or PDF.

This is useful when preparing:

- Research packages.
- Writing drafts.
- Project notes.
- Edited documents for use outside InsightCAP.

## Uninstall Data Choices

The uninstaller is designed to avoid deleting user data by default.

Optional uninstall choices may include:

- Delete local app data, including settings, cache files, downloaded models, and
  the default knowledge base.
- Clear the local decryption key for the current Windows user account.
- Permanently delete the selected custom knowledge folder.

These options are intentionally opt-in. Users should only choose destructive
options when they understand the impact.

If the encrypted knowledge base is kept but the local decryption key is cleared,
the 24-word recovery phrase is required to unlock the knowledge base after
reinstallation.
