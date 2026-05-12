# Contributing to Zileo Chat

Thanks for taking the time to contribute. This document covers the
contributor workflow only.

## Prerequisites

- Node.js >= 20.19.0 (22.12+ or 24+ recommended)
- Rust toolchain (rustc >= 1.80.1)
- A Linux desktop environment for running the Tauri app

```bash
git clone https://github.com/<your-fork>/Zileo-Chat-3.git
cd Zileo-Chat-3
npm install
cd src-tauri && cargo build && cd ..
npm run tauri:dev
```

## Branch and commit convention

- Branch from `main`. Use a descriptive prefix: `feature/`, `fix/`,
  `chore/`, `docs/`, `refactor/`, `security/`.
- Keep commits focused. Conventional Commits style is appreciated but
  not enforced (e.g. `fix(memory): handle empty tags array`).
- One topic per PR. Mixed changes make review slower.

## Required checks before opening a PR

Every PR must keep these green locally. CI runs the same checks and
will block the merge otherwise.

```bash
# Frontend
npm run format:check
npm run lint
npm run check
npm run test

# Backend (run sequentially, never in parallel)
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## CHANGELOG

This project follows the [Keep a Changelog](https://keepachangelog.com/)
convention with a single `## [Unreleased]` section at the top of
`CHANGELOG.md`. **Contributors add their entries there.**

When your change is user-visible or noteworthy, add a short bullet under
the appropriate subsection (`Added`, `Changed`, `Fixed`, `Removed`,
`Security`, or a custom one like `Tooling`):

```markdown
## [Unreleased]

### Fixed

- Memory chunk cascade: orphan chunks now removed when their parent
  workflow is deleted. Adds two TDD tests that lock the contract.
```

Purely internal refactors with no user-visible behavior change are
exempt from a CHANGELOG entry, but a one-line note is still appreciated.

## What contributors must NOT touch

The project version is managed by the maintainer **at release time
only**. Please do not modify any of these files:

- `package.json` (`"version"` field)
- `package-lock.json` (`"version"` field)
- `src-tauri/Cargo.toml` (`version = ...`)
- `src-tauri/Cargo.lock` (`zileo-chat` entry)
- `src-tauri/tauri.conf.json` (`"version"` field)
- `README.md` (version badge and beta warning)

If any of these files show a version change in your diff, please revert
the version line. The maintainer will perform a single atomic version
bump across all of them when cutting the next release, and contributor
bumps would conflict with that flow.

You also do not need to regenerate `Cargo.lock` if you add a Rust
dependency to `Cargo.toml`. CI regenerates it automatically and the
maintainer commits the up-to-date lockfile on `main` when needed.

## Opening the PR

- Target `main`.
- PR title: short and descriptive. Titles surface in the GitHub release
  notes once accumulated, so keep them readable on their own.
- PR body: a short summary of intent, the visible behavior change if
  any, and a checklist of what you tested.
- Link any related issue.

## Code style cheat sheet

- TypeScript: use `$types/<module>` for type imports, never
  `$lib/types`. No `any`, no `// @ts-ignore`.
- Svelte: runes only (`$props`, `$state`, `$derived`, `{#snippet}`,
  `{@render}`, `{@attach}`). No `export let`, no `<slot>`, no
  `on:click`.
- Zod: v4 API only (`error` not `message`, `z.email()`, two-argument
  `z.record(k, v)`).
- Rust: no `.unwrap()` or `.expect()` in production code. Use `Result`
  and `.map_err(|e| format!("..."))?` in Tauri commands.
- IPC: TypeScript camelCase parameters become Rust snake_case
  automatically. No manual conversion needed.
- `Option<T>` with `#[serde(skip_serializing_if = "Option::is_none")]`
  maps to TypeScript `field?: T`. Without skip, it maps to
  `field: T | null`.
- Errors: `getErrorMessage(e)` from `$lib/utils/error` in TypeScript;
  `.map_err(|e| ...)` in Rust.

## License

By contributing, you agree that your contributions will be licensed
under the Apache License 2.0, the same license as this project. See
`LICENSE` for the full text.
