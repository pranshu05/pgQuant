# Contributing to pgQuant

Thank you for your interest in contributing to `pgQuant`! This project brings quantitative finance primitives to PostgreSQL using Rust and `pgrx`.

## Getting Started

1. **Install Prerequisites**: You'll need Rust, `cargo-pgrx`, and PostgreSQL development headers (or let `cargo pgrx init` download them). See the README for the exact `cargo-pgrx` version to install.
2. **Build and Test**: Run `cargo pgrx test` to ensure the extension builds and all tests pass on your machine.

## Development Workflow

1. Discuss major changes in an issue before writing code.
2. Fork the repository and create a feature branch.
3. Ensure you add tests (both Rust unit tests and `#[pg_test]` integration tests) for any new logic.
4. Update documentation in `docs/` and inline comments as needed.

## Good First Issues

If you're looking for a place to start, consider contributing to tasks we haven't implemented yet from our roadmap, such as:
- Adding additional VaR methods (e.g., Cornish-Fisher).
- Implementing more shrinkage targets for covariance matrices.
- Expanding the return models to account for FX or crypto market quirks.

Please check the issues tab for items marked "good first issue".