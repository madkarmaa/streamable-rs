# Development workflow

The pre-commit hook runs `cargo check -p streamable-rs --no-default-features`
before workspace tests and Clippy whenever Rust files, any Cargo manifest, or
`Cargo.lock` are staged. This keeps the runtime-neutral core configuration
continuously build-checked, including dependency-only commits.
