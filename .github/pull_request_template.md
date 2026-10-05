## Description
<!-- Please include a summary of the quantitative model or Rust/pgrx changes made. Please also include relevant motivation and context. -->

Fixes # (issue)

## Type of change
<!-- Please delete options that are not relevant. -->

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New quantitative model/feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing SQL interfaces to change)
- [ ] Documentation update (e.g., adding to `docs/`)

## How Has This Been Tested?
<!-- Please describe the tests that you ran to verify your changes. Include SQL commands to test the function manually, or confirm that you have added Rust integration/unit tests. -->

- [ ] Added `#[test]` / `#[pg_test]` to verify mathematical correctness.
- [ ] Tested manually against a sample dataset in PostgreSQL.

## Checklist:

- [ ] My code follows the Rust standard formatting (`cargo fmt`).
- [ ] I have run clippy (`cargo clippy`) and resolved any warnings.
- [ ] I have performed a self-review of my code, paying special attention to memory safety around PostgreSQL bindings.
- [ ] I have commented my code, particularly in complex math or unsafe blocks.
- [ ] I have made corresponding changes to the markdown documentation in `docs/`.
- [ ] I have added/updated tests that prove my mathematical models are accurate against standard benchmarks (e.g., Python/pandas/scipy).
- [ ] Existing `cargo test` suites pass locally with my changes.
