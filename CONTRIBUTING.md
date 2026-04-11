# Contributing to Ajaya

First off, thank you for considering contributing to **Ajaya (अजय)**! Every contribution helps make this framework more unconquerable. 🔱

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Commit Messages](#commit-messages)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Features](#suggesting-features)
- [License](#license)

---

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).
By participating, you are expected to uphold this code.

To report unacceptable behavior, use one of these channels:
- **Discord:** Join the [Aarambh Dev Hub Discord](https://discord.gg/) and open a private message to the maintainer
- **GitHub:** Open a [private security advisory](https://github.com/AarambhDevHub/ajaya/security/advisories/new) for sensitive reports

---

## Getting Started

Ajaya is a Rust workspace with 12 crates. Before contributing, please familiarize yourself with:

- [ARCHITECTURE.md](ARCHITECTURE.md) — Full technical specification
- [ROADMAP.md](ROADMAP.md) — Version-by-version development plan
- [CHANGELOG.md](CHANGELOG.md) — What has changed so far

---

## Development Setup

### Prerequisites

- **Rust 1.85+** — Install via [rustup](https://rustup.rs/)
- **Git** — For version control
- **curl** — For manual testing (optional)

### Building

```bash
# Clone the repository
git clone https://github.com/AarambhDevHub/ajaya.git
cd ajaya

# Build all crates
cargo build --workspace

# Run all checks (what CI runs)
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo fmt --all -- --check
```

### Running

```bash
# Start the development server
cargo run -p ajaya

# Test it
curl http://localhost:8080
```

---

## How to Contribute

### 1. Find Something to Work On

- Check the [ROADMAP.md](ROADMAP.md) for the current milestone
- Look for [open issues](https://github.com/AarambhDevHub/ajaya/issues) tagged `good first issue` or `help wanted`
- Check if the feature you want is already planned in the roadmap
- Ask in the [Aarambh Dev Hub Discord](https://discord.gg/) if you're unsure where to start

### 2. Fork and Branch

```bash
# Fork the repo on GitHub, then:
git clone https://github.com/YOUR_USERNAME/ajaya.git
cd ajaya
git checkout -b feature/your-feature-name
```

### 3. Make Your Changes

- Write code following our [coding standards](#coding-standards)
- Add tests for new functionality
- Update documentation if needed
- Keep changes focused — one feature/fix per PR

### 4. Test Your Changes

```bash
# Run the full CI suite locally
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo fmt --all -- --check
```

### 5. Submit a Pull Request

See [Pull Request Process](#pull-request-process) below.

---

## Pull Request Process

1. **Update documentation** — If your change affects the public API, update relevant docs
2. **Add changelog entry** — Add a note under `[Unreleased]` in `CHANGELOG.md`
3. **Pass CI** — All checks must pass: `check`, `clippy`, `test`, `fmt`
4. **Write a clear description** — Explain what, why, and how
5. **Link issues** — Reference related issues with `Closes #123` or `Fixes #456`
6. **Request review** — Tag maintainers for review
7. **Address feedback** — Respond to review comments promptly

### PR Title Format

Use conventional commit style:

```
feat(router): add radix trie path matching
fix(hyper): handle connection reset gracefully
docs(readme): update quick start example
test(extract): add Path<T> deserialization tests
refactor(core): simplify Body type alias
ci: add benchmark workflow
chore: update dependencies
```

---

## Coding Standards

### Rust Style

- Follow the official [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` with default settings (no custom `rustfmt.toml`)
- Zero `cargo clippy` warnings — CI enforces `-D warnings`
- Use `#[must_use]` on functions that return important values
- Use `#[inline]` sparingly — only on small, hot-path functions

### Documentation

- Every `pub` item must have a `///` doc comment
- Every crate must have a `//!` top-level doc comment
- Include `# Examples` sections for major types and functions
- Use `# Panics`, `# Errors`, `# Safety` sections where appropriate

### Error Handling

- Use `thiserror` for library error types
- Never use `.unwrap()` in library code (only in tests and examples)
- Provide meaningful error messages that help users debug issues

### Performance

- Zero heap allocations on the hot path (routing, handler dispatch)
- Use `Bytes` and `BytesMut` instead of `Vec<u8>` for body data
- Benchmark before and after performance-related changes

### Testing

- Unit tests go in the same file as the code (`#[cfg(test)] mod tests`)
- Integration tests go in `tests/` directory of each crate
- Use descriptive test names: `test_path_param_extraction_with_uuid`
- Test both success and failure paths

---

## Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `test` | Adding or updating tests |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `perf` | Performance improvement |
| `ci` | CI/CD changes |
| `chore` | Maintenance tasks (dependencies, tooling) |
| `breaking` | Breaking API change |

### Scopes

Use the crate name without the `ajaya-` prefix: `core`, `router`, `hyper`, `extract`, `middleware`, `ws`, `sse`, `static`, `tls`, `macros`, `test`.

---

## Reporting Bugs

### Before Reporting

1. Check existing [issues](https://github.com/AarambhDevHub/ajaya/issues) — it might already be reported
2. Try the latest version — the bug might be fixed
3. Create a minimal reproduction case

### Bug Report Template

```markdown
## Description
A clear description of the bug.

## Steps to Reproduce
1. Create a handler with...
2. Register route...
3. Send request...

## Expected Behavior
What you expected to happen.

## Actual Behavior
What actually happened.

## Environment
- Ajaya version: 0.0.1
- Rust version: 1.85
- OS: Linux / macOS / Windows
```

---

## Suggesting Features

1. Check the [ROADMAP.md](ROADMAP.md) — it might already be planned
2. Open a [GitHub Discussion](https://github.com/AarambhDevHub/ajaya/discussions) first for larger features
3. For smaller features, open an issue with the `enhancement` label
4. Chat about ideas in the [Aarambh Dev Hub Discord](https://discord.gg/) `#ajaya` channel

---

## Workspace Structure

When contributing, it helps to know where things live:

| Crate | What it does | Status |
|-------|-------------|--------|
| `ajaya` | Facade — re-exports everything | Active |
| `ajaya-core` | Request, Response, Body, Error | Active |
| `ajaya-hyper` | Hyper server integration | Active |
| `ajaya-router` | Radix trie routing | Stub |
| `ajaya-extract` | Extractors (Path, Query, Json) | Stub |
| `ajaya-middleware` | CORS, compression, timeout | Stub |
| `ajaya-ws` | WebSocket | Stub |
| `ajaya-sse` | Server-Sent Events | Stub |
| `ajaya-static` | Static file serving | Stub |
| `ajaya-tls` | TLS / HTTPS | Stub |
| `ajaya-macros` | Proc macros | Stub |
| `ajaya-test` | Test utilities | Stub |

---

## Community

- 💬 **Discord:** [Aarambh Dev Hub](https://discord.gg/) — `#ajaya` channel for questions and discussion
- 🐙 **GitHub Discussions:** [github.com/AarambhDevHub/ajaya/discussions](https://github.com/AarambhDevHub/ajaya/discussions)
- 📺 **YouTube:** [Aarambh Dev Hub](https://youtube.com/@AarambhDevHub) — follow the build in public series

---

## License

By contributing to Ajaya, you agree that your contributions will be licensed under
the same dual license as the project:

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

---

*Thank you for helping make Ajaya unconquerable!* 🔱