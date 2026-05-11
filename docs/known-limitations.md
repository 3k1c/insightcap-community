# Known Limitations

InsightCAP Community v0.9.0-beta.1 is a beta release. Capture, extraction, AI,
and automation workflows may depend on the source format, local environment,
network availability, configured services, and third party platform behavior.

## Web Page Capture

InsightCAP supports extracting readable content from many standard web pages,
but dynamic web pages may be only partially supported.

Pages that rely heavily on client-side rendering, login sessions, infinite
scrolling, embedded apps, anti-bot protection, or paywalls may not extract
complete content.

## Video URL Capture

Supported video URL extraction is best-effort and may depend on subtitle
availability, platform metadata, network access, and local helper tools.

YouTube and Bilibili behavior may change over time. Extraction may fail or
return partial metadata when platform access, subtitle availability, or helper
tool behavior changes.

## Document Capture

Document extraction depends on the file type and document structure. Complex
layouts, scanned documents, image-only PDFs, protected files, or unusual
encodings may require OCR or may produce incomplete text.

## AI-assisted Workflows

AI text editing, summarization, and knowledge Q&A depend on the configured AI
service and model. Output quality, latency, and availability may vary.

## Telegram Bot Workflows

Telegram Bot features require a configured bot token, allowed user settings, and
network access. Telegram platform behavior and message size limits may affect
capture and response behavior.

## Local Data and Recovery

The encrypted local knowledge base requires the local decryption key or the
saved 24-word recovery phrase. If both are unavailable, encrypted knowledge base
data cannot be recovered.
