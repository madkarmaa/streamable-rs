## Project

`streamable-rs` is a Rust rewrite of the sibling `streamable-py` project.

The primary goal is behavioral parity with the inspected Python implementation while exposing an idiomatic Rust API. Streamable's API is undocumented, so exact wire behavior matters more than assumptions based on endpoint naming or conventional REST behavior.

Repository layout:

- `core/` — library crate (`streamable`)
- `cli/` — CLI crate
- `../streamable-py/` — sibling Python implementation and behavioral reference, when available

---

## Agent bootstrap

These steps are mandatory before normal project work:

1. **Read the `caveman` skill first.** Locate the project-local `caveman` skill under `.agents/skills/` and read its instructions before inspecting, editing, testing, or committing project code.
2. **Ensure the repository dump exists.** If the repository-root `dump/` directory is missing, run the repository-root `dump.py` before proceeding. Do not regenerate `dump/` merely because it exists unless the current task requires it.

---

## Browser automation

When a task requires interacting with Chrome, inspecting a live page, reproducing browser behavior, or using browser developer tooling:

1. **Use the runtime's built-in Chrome/browser-control API first.** Prefer the built-in browser automation path for normal navigation, interaction, inspection, and verification.
2. **Fall back to the `chrome-devtools` MCP only if the built-in browser API fails, is unavailable, cannot attach to the required page/session, or lacks a capability needed for the task.**
3. Do not skip directly to the MCP unless the user explicitly requests it or the task is known to require a DevTools-only capability that the built-in API cannot provide.
4. When falling back after a partial browser action, inspect the current state before repeating actions. Avoid duplicating submissions, mutations, uploads, account changes, or other side effects.

---

## `.agents` protocol and persistent memories

Treat the repository-local `.agents/` directory as version-controlled, portable agent state and follow the `.agents` protocol for its structure and file formats.

### Memory maintenance

During and after project work, maintain durable project knowledge under `.agents/memories/` when new information is worth carrying into future sessions.

Memory entries should capture reusable facts such as:

- architectural or protocol decisions;
- verified behavior and invariants;
- recurring implementation or testing pitfalls;
- user/project workflow preferences;
- discoveries that would otherwise need to be rediscovered.

Do not turn memories into a transcript or duplicate short-lived task state. Update an existing relevant memory instead of creating overlapping entries when practical.

**Portability is mandatory.** Memories must remain repository- and platform-portable:

- never store absolute/full filesystem paths;
- never store usernames, home directories, drive letters, host-specific locations, current working directories, or similar environment data;
- never store local machine configuration, transient environment details, secrets, credentials, tokens, or other host-specific state;
- when a file reference is useful, prefer a repository-relative reference;
- describe commands and behavior in a platform-neutral way when possible;
- do not encode assumptions that only hold for one developer's checkout or operating system unless that platform dependency is itself part of the project's intentional contract.

After memories are updated, commit the memory changes in a dedicated commit with the exact subject:

`chore(agents): update memories`

This memory commit is an explicit exception to the general rule that commits are only made when the user asks: once project memories have been updated, commit them. Keep unrelated source changes out of this commit.

### Other `.agents` artifacts

The `.agents` protocol permits more than memories. Add other project-local, spec-compliant agent artifacts when they materially improve future work, including skills, agent definitions, tasks, prompts/configuration, or other structures supported by the current specification.

Apply the same portability rule to those artifacts: avoid machine-specific state, absolute paths, secrets, and unnecessary environment assumptions. Keep additions minimal, purposeful, and version-control friendly.

---

## Operating principles

### 1. Treat the Python project as the parity reference

Before implementing or changing a Streamable feature:

1. Inspect the corresponding code in `../streamable-py`.
2. Check request models, response models, endpoint construction, error handling, and call order.
3. Preserve wire-visible behavior unless the user explicitly asks for a Rust-specific behavior change.
4. Do not infer undocumented endpoint paths from a base URL or neighboring endpoints.
5. If the Python behavior is ambiguous, obsolete, or contradicted by live behavior, call that out rather than silently inventing a new contract.

### 2. Distinguish stable parity from live-service facts

The API is undocumented and can drift.

Treat these as candidates for live revalidation when integration behavior matters:

- endpoint availability
- response shapes not covered by fixtures
- free-plan limits
- authentication behavior
- upload/S3 behavior
- undocumented error responses

Do not revalidate by sending live requests during ordinary/default tests. Live checks must follow the remote-test policy below.

---

## Error handling

Preserve endpoint-specific domain errors already established by the Rust client.

Do not mechanically reproduce Python's accidental error behavior when an idiomatic Rust typed error already exists.

When adding a new endpoint:

1. inspect Python's exact status/body handling;
2. add deterministic local tests for mapped statuses;
3. make the Rust failure mode explicit;
4. avoid broad catch-all behavior unless compatibility requires it.

---

## Testing policy

### Default tests must be offline

Running normal tests must not contact Streamable, create accounts, mutate labels, upload files, or otherwise send requests to the live service.

The explicit opt-in feature is:

`DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER`

Remote tests must be guarded with:

`#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]`

### Every practical client/API behavior gets two layers of coverage

For each implemented client/API feature:

1. add deterministic local/mock coverage;
2. add the smallest practical feature-gated remote test.

Exceptions:

- rate-limit tests;
- cases that would intentionally spam, burden, or repeatedly mutate the service;
- cases where cleanup is impossible and the live side effect is disproportionate.

For remote mutation tests:

- serialize through the existing `REMOTE_TEST_LOCK`;
- make the minimum number of requests;
- do not repeatedly retry;
- clean up created resources when the API supports cleanup.

Rate-limit behavior is local-only. Never deliberately trigger remote rate limits.

### Mocking rules

When using WireMock or an equivalent mock server:

- assert the effective full path produced by the configured base URL;
- assert HTTP method;
- assert meaningful request bodies and wire names;
- assert bodylessness where relevant, especially DELETE;
- test status-to-domain-error mappings.

---

## Validation before declaring work complete

For Rust changes, run the relevant targeted tests first, then the full quality gate.

Preferred full validation:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
```

If a task specifically changes feature-gated remote behavior, also run the remote suite only when explicitly appropriate:

```sh
cargo test --workspace --features DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER
```

Do not run the remote suite casually. It is intentionally dangerous/side-effecting.

The project uses strict Clippy settings. Run Clippy early enough that lint-driven design changes do not accumulate at the end.

---

## Git and commit discipline

The user strongly prefers atomic, behavior-specific commits.

The `.agents` memory workflow is a standing exception to the normal "commit only when asked" rule: after updating `.agents/memories/`, create the dedicated `chore(agents): update memories` commit described above.

When asked to commit:

1. partition changes by externally observable behavior;
2. split dependencies, implementation, ignore rules, refactors, and unrelated fixes when they are independently meaningful;
3. split down to hunks when necessary;
4. before each commit, inspect:
    - `git diff --cached --stat`
    - `git diff --cached`
    - `git diff --cached --check`
5. ensure the commit can be understood and reverted independently.

Do not make a broad commit simply because all changes belong to the same task.

If a commit is too broad, prefer a soft undo such as:

`git reset --soft HEAD^`

then repartition the staged changes.

Generated verification/rollback artifacts under `.codex/` should remain untracked/ignored unless the user explicitly asks to commit them.

Do not push or rewrite published history unless the user explicitly asks.

---

## How to approach a new feature

For a new Streamable API/client feature, use this sequence unless the user gives a different order:

1. inspect the sibling Python implementation;
2. identify exact endpoint, method, payload, aliases, success shape, and error mapping;
3. identify whether authentication is required;
4. design the Rust API so fixed protocol details remain internal;
5. add request/response/error models;
6. add deterministic local/mock tests;
7. implement the behavior;
8. run targeted tests and Clippy;
9. add bounded feature-gated remote coverage when practical;
10. run full formatting/tests/Clippy/diff checks;
11. if asked to commit, create behavior-specific atomic commits.

When the user specifies an implementation/validation/commit order, follow that order exactly rather than batching the whole feature set first.

---

## Priority when instructions conflict

Use this precedence:

1. the user's current explicit instruction;
2. current repository behavior/tests and explicit project documentation;
3. the currently inspected sibling Python implementation for parity questions;
4. this `AGENTS.md`;
5. historical memories.

If an undocumented live API contradicts the remembered contract, report the discrepancy and update tests/behavior only in line with the user's requested compatibility goal.

Do not silently "fix" parity differences just because another design appears cleaner.
