# InsightCAP Architecture

This document provides a public architecture overview for InsightCAP Community.
It is written for users, reviewers, and community contributors. It intentionally
stays at an external-facing level and does not include private source code,
internal policies, proprietary algorithms, prompt policies, or Commercial
Edition implementation details.

## Overview

InsightCAP is a local-first desktop application designed to help users capture,
organize, search, and revisit their own knowledge.

The Community Edition focuses on:

- Providing a controlled desktop knowledge workflow.
- Keeping local data ownership and local-first usage as a design direction.
- Explaining the product architecture and usage model clearly.
- Publishing selected community-facing source code and technical documentation.
- Preserving room for future Commercial Edition features, proprietary modules,
  and enterprise capabilities.

## Technology Stack

The public technical direction for InsightCAP Community includes:

- Desktop runtime: Tauri.
- Frontend: React, TypeScript, and Vite.
- UI state: Zustand.
- Editor layer: TipTap.
- Styling and UI utilities: Tailwind CSS and lucide-react.
- Testing direction: Vitest and React Testing Library.

This repository contains only public source code, documentation, and release
materials. The complete private implementation is not included.

## High-Level Architecture

At a high level, InsightCAP can be described as:

```text
User
  |
  v
Desktop App
  |
  +-- User Interface
  |
  +-- Local Application Services
  |
  +-- Capture and Knowledge Workflows
  |
  +-- Local Storage Layer
  |
  +-- Release and Update Channel
```

### Desktop App

The desktop app provides the main user entry point, including the application
window, desktop integration, file selection, shortcuts, and local runtime
behavior.

Community documentation describes this layer conceptually. It does not publish
private command wiring, internal build policy, or Commercial Edition details.

### User Interface

The user interface presents the main workflows, including:

- Capture.
- Review.
- Search.
- Knowledge organization.
- Editor-based content editing.
- Settings and local preferences.

The UI layer is based on React and TypeScript, with supporting state management
and editor components.

### Local Application Services

Local application services coordinate interactions between the UI and local
capabilities. This layer may handle:

- User workflow state.
- Local application state updates.
- Content organization.
- Search, export, reminder, and follow-up entry points.
- Boundaries between the desktop runtime and the user interface.

Public documentation describes the service boundaries, but does not disclose
proprietary logic or private business rules.

### Capture and Knowledge Workflows

InsightCAP is centered around capture, organization, and recall. The public
architecture describes the flow conceptually:

```text
Input Content
  |
  v
Capture Flow
  |
  v
Review and Organization
  |
  v
Local Knowledge Store
  |
  v
Search, Recall, Export, or Follow-up
```

This flow explains the product direction. It does not imply that this repository
contains the complete production implementation.

### Local Storage Layer

InsightCAP follows a local-first direction. Public architecture documentation
emphasizes that:

- User data should remain understandable and user-controlled where applicable.
- Local caches, databases, environment files, and generated artifacts should not
  be committed to the public repository.
- Public releases must be reviewed for personal data and local development
  metadata.

The public repository should not include:

- User databases.
- Local caches.
- Embedding caches.
- `.env` files.
- Private tokens.
- Machine-specific metadata.
- Developer local paths.

## Release Model

Community Edition installers and binary artifacts should be distributed through
GitHub Releases, not committed directly to Git history.

The repository may include:

- Installation guides.
- Release notes.
- Checksums or verification notes, if available.
- Public changelogs.
- License and usage scope documentation.

## Licensing Model

InsightCAP Community uses a two-layer licensing model:

- Source code and documentation in this repository are licensed under the MIT
  License unless otherwise stated.
- Community Edition binaries are governed by the Community Edition License.
- Community Edition binaries are free to use for personal and commercial
  purposes.
- Commercial Edition features, proprietary modules, hosted services, enterprise
  licensing, and private source code are not included in the Community Edition
  License.

See:

- `LICENSE`
- `COMMUNITY-LICENSE.md`
- `docs/open-source-scope.md`

## Public and Private Boundary

### Included in This Public Repository

This repository may include:

- Public README.
- High-level architecture documentation.
- Installation guide.
- Privacy and local-data notes.
- Community Edition license.
- Selected non-sensitive source code.
- Example workflows or simplified modules.
- Public release notes.

### Not Included in This Public Repository

This repository should not include:

- Private source code.
- Proprietary modules.
- Commercial Edition features.
- Internal prompts or policies.
- Private roadmap items.
- Internal debugging notes.
- User data.
- Local databases.
- Cache files.
- API keys, tokens, or credentials.
- Personal email addresses.
- Local Windows usernames or machine names.
- Operating system build details from development machines.

## Security and Privacy Review

Before making this repository public, each release should pass a basic
public-readiness review:

1. Scan source and documentation for personal information.
2. Confirm Git history does not contain private email addresses or
   machine-specific metadata.
3. Ensure installers are distributed through GitHub Releases.
4. Confirm local cache and database files are excluded.
5. Confirm proprietary implementation details are not included.
6. Confirm the license scope is clearly stated.

## Future Direction

InsightCAP Community is intended to provide a transparent public entry point for
users and contributors.

Future public materials may include:

- More detailed installation documentation.
- Community-facing examples.
- Selected source modules.
- Public issue templates.
- Contribution guidelines.
- Release verification notes.

Commercial Edition capabilities may be introduced separately in the future and
are outside the scope of this community repository.
