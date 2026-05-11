# File and Source Support

InsightCAP Community can capture and extract content from several source types
for review, editing, search, and long-term personal memory workflows.

Support is best-effort in this beta release. Extraction quality may depend on
file structure, source availability, local environment, configured services, and
third party platform behavior.

## Supported Source Categories

InsightCAP Community is intended to support these capture categories:

- Web pages.
- Local documents.
- Supported video URLs.
- Clipboard content.
- Notes and structured text.
- Telegram Bot capture inputs, when configured.

Captured content should be reviewed before it is saved or reused as knowledge.

## Web Pages

InsightCAP can extract readable content from many standard web pages.

Web page capture works best when the page exposes article text, document text,
or readable page content in the loaded HTML.

Limited support should be expected for:

- Heavily dynamic web pages.
- Pages that require login sessions.
- Infinite scrolling pages.
- Embedded web apps.
- Paywalled pages.
- Pages protected by anti-bot systems.
- Pages where important content is rendered only after complex user actions.

Dynamic pages may extract partial content or metadata only.

## Documents

Document extraction depends on file type, document structure, encoding, and
layout complexity.

Extraction works best for text-based documents with selectable text.

More limited results should be expected for:

- Scanned documents.
- Image-only PDFs.
- Protected or encrypted files.
- Documents with complex multi-column layouts.
- Files with unusual encodings.
- Documents where text is embedded as images.

Some document types or scanned files may require OCR support or manual review.

## Video Content

InsightCAP can work with supported video URLs such as YouTube and Bilibili.

Video URL capture is best-effort and may depend on:

- Subtitle or transcript availability.
- Public access to the video.
- Platform metadata availability.
- Network access.
- Local helper tool behavior.
- Changes made by the video platform.

If subtitles or transcript data are unavailable, extraction may return partial
metadata or fail.

## Clipboard, Notes, and Structured Text

Clipboard and note capture are suitable for quick manual input, short excerpts,
drafts, copied research material, and structured text prepared by the user.

Users should review captured text before saving it, especially when the source
contains formatting, code blocks, tables, or mixed languages.

## Telegram Bot Capture Inputs

When Telegram Bot workflows are configured, messages can be used as lightweight
capture inputs.

Telegram capture behavior may depend on:

- Bot configuration.
- Allowed user settings.
- Network availability.
- Telegram message limits.
- File or media type support.

Users should avoid sending sensitive content through any workflow unless they
understand their configured environment and data handling choices.

## AI-assisted Processing

AI-assisted cleanup, summarization, rewriting, and editing may be used after
content is captured.

AI output quality depends on the configured model and service. Users should
review AI-generated text before saving, exporting, or relying on it.

## Export

The AI text editor can export the current document to common document formats
such as TXT, Markdown, HTML, DOCX, and PDF.

Export results may depend on document structure, formatting, and the target
format.

## Review Requirement

Captured and extracted content should be treated as draft material until
reviewed.

Users should confirm that:

- Important text was extracted correctly.
- Source context is still accurate.
- AI-edited text has not changed the intended meaning.
- Exported files preserve the expected formatting.
