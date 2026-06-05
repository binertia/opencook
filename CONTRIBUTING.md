# Contributing to AI Gateway

Thank you for your interest in contributing! This document outlines the development workflow and conventions we follow.

## Development Setup

### Prerequisites

- Rust 1.78+ (see `rust-toolchain.toml`)
- Node.js 20+ (for frontend)
- PostgreSQL 16+ (for TEAM mode tests)
- Redis 7+ (for caching / rate limiting tests)
- `sqlx-cli` for migrations

### Quick Start

```bash
# Clone the repository
git clone https://github.com/ai-gateway/ai-gateway
cd ai-gateway

# Install pre-commit hooks
cargo install lefthook
lefthook install

# Run in SOLO mode (zero config)
cargo run --bin opencook -- serve

# Run tests
cargo test --workspace --lib

# Run E2E tests (requires PostgreSQL + Redis)
cargo test -p gateway-api --test e2e
```

## Code Style

### Rust

- Format with `cargo fmt` (config in `rustfmt.toml`)
- Lint with `cargo clippy -- -D warnings` (config in `.clippy.toml`)
- Avoid `unwrap()` and `expect()` in production code; use `?` or explicit error handling
- Document all public APIs with doc comments

### TypeScript / Frontend

- Format with Prettier (config in `frontend/.prettierrc`)
- Lint with ESLint (config in `frontend/.eslintrc.cjs`)
- Use React functional components with hooks
- Prefer `ky` over `fetch` for API calls

## Testing

- All new features must include unit tests
- Integration tests live in `crates/*/tests/` and `tests/e2e_*.rs`
- Run the full test suite before opening a PR:

```bash
cargo test --workspace --lib
# Frontend
cd frontend && npm test -- --run
```

## Pull Request Process

1. Fork the repository and create a feature branch
2. Ensure pre-commit hooks pass (`lefthook run pre-commit`)
3. Update documentation if your change affects configuration, API, or architecture
4. Fill out the pull request template checklist
5. Request review from a maintainer

## Commit Messages

- Use present tense ("Add feature" not "Added feature")
- Use imperative mood ("Move cursor to..." not "Moves cursor to...")
- Keep the first line under 72 characters

## Getting Help

- Open a [discussion](https://github.com/ai-gateway/ai-gateway/discussions) for questions
- Open an [issue](https://github.com/ai-gateway/ai-gateway/issues) for bugs or feature requests
