# Streamable

`streamable` is a Rust client for Streamable's undocumented API, with the library in `core/` and the CLI in `cli/`.

The API is undocumented. Exact wire behavior matters more than assumptions based on endpoint names, REST conventions, or what the service _should_ do. When the implementation, fixtures, and live service disagree, treat that disagreement as useful evidence rather than smoothing it over.

## What matters most

### 1. Preserve observed protocol behavior

Streamable can drift, and undocumented behavior is part of the contract we are reverse-engineering.

Treat these as candidates for live revalidation when integration behavior matters:

- endpoint availability
- response shapes not covered by fixtures
- free-plan limits
- authentication behavior
- upload/S3 behavior
- undocumented error responses

Do not turn ordinary tests into live probes. Remote verification follows the explicit remote-test policy below.

### 2. Keep the core deterministic

Normal tests are offline. Client/API behavior should be reproducible against mocks without needing Streamable, an account, uploaded files, or other live state.

For each practical API/client feature, aim for two layers of coverage:

- deterministic local/mock coverage
- the smallest practical feature-gated remote test

Rate-limit tests, intentionally abusive cases, and side effects that cannot be cleaned up proportionally are local-only.

### 3. Prefer small, explicit changes

The user strongly prefers atomic, behavior-specific commits. Do not bundle unrelated implementation, dependency, refactor, ignore-rule, or cleanup changes merely because they were discovered during the same task.

When a change can be understood and reverted independently, it should usually stand independently.

## Before you start

The repository uses a generated repository dump as working context.

If the repository-root `dump/` directory does not exist, run the repository-root `dump.py` before normal project work. Do not regenerate an existing dump merely because it exists unless the current task actually requires a fresh one.

## Browser work

When a task requires Chrome, a live page, browser inspection, reproduced web behavior, or developer tooling:

1. Use the runtime's built-in browser-control API first.
2. Fall back to the `chrome-devtools` MCP only when the built-in path fails, is unavailable, cannot attach to the required page/session, or lacks a required capability.
3. Do not skip directly to the MCP unless the user explicitly asks for it or the task is known to require a DevTools-only capability.
4. After a partial browser action, inspect the current state before retrying. Do not accidentally duplicate submissions, uploads, mutations, account changes, or other side effects.

## `.agents` is durable project memory

Treat `.agents/` as portable agent state and follow the `.agents` protocol for supported structures and file formats.

### Memories

Maintain reusable project knowledge under `.agents/memories/` when a discovery is worth carrying into future sessions.

Good memories include:

- architectural or protocol decisions
- verified behavior and invariants
- recurring implementation or testing pitfalls
- project workflow preferences
- discoveries that would otherwise need to be rediscovered

Do not use memories as a transcript or as storage for short-lived task state. Prefer updating an existing relevant memory over creating overlapping entries.

Anything stored under `.agents/` must remain portable across machines, checkouts, users, and operating systems. In particular, memories must:

- use repository-relative file references
- avoid absolute/full filesystem paths
- avoid usernames, home directories, drive letters, current working directories, and host-specific locations
- avoid local machine configuration, transient environment state, secrets, credentials, and tokens
- describe commands and behavior in a platform-neutral way when practical
- avoid assumptions that only happen to be true on the current host
- only encode a platform dependency when that dependency is intentionally part of the project contract

After changing `.agents/memories/`, commit those memory changes separately with the exact subject:

```text
chore(agents): update memories
```

This is the standing exception to the normal rule that commits are only created when the user asks. Keep unrelated source changes out of the memory commit.

### Other `.agents` artifacts

The protocol is not limited to memories. Add other spec-compliant project-local artifacts—skills, agent definitions, tasks, prompts/configuration, or other supported structures—when they materially improve future work.

Keep them minimal, purposeful, portable, and friendly to version control.

## The dangerous part: remote tests

Normal tests must never contact Streamable, create accounts, mutate labels, upload files, or otherwise send requests to the live service.

The explicit opt-in feature is:

```text
DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER
```

Remote tests must be guarded with:

```rust
#[cfg(feature = "DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER")]
```

For remote mutation tests:

- serialize through the existing `REMOTE_TEST_LOCK`
- make the minimum number of requests
- do not repeatedly retry
- retain created resources instead of issuing cleanup requests
- send a remote `DELETE` only when deletion itself is the behavior under test, never as cleanup for another test or as a fallback after failure

Never deliberately trigger remote rate limits.

## Mock the wire, not your assumptions

When using WireMock or an equivalent mock server, verify the effective protocol behavior that matters:

- full path produced by the configured base URL
- HTTP method
- meaningful request bodies and exact wire names
- bodylessness where relevant, especially `DELETE`
- status-to-domain-error mappings

Mocks should protect observed API behavior, not merely prove that some request was sent.

## Error handling

Preserve endpoint-specific domain errors already established by the Rust client.

When adding a new endpoint:

1. identify the relevant status/error behavior
2. add deterministic local coverage for mapped statuses
3. make the Rust failure mode explicit
4. avoid broad catch-all behavior unless compatibility requires it

## Adding a new API feature

Unless the user gives a different order, use this sequence:

1. identify the exact endpoint, method, payload, aliases, success shape, and error mapping
2. determine whether authentication is required
3. design the Rust API so fixed protocol details stay internal
4. add request/response/error models
5. add deterministic local/mock tests
6. implement the behavior
7. run targeted tests and Clippy
8. add bounded feature-gated remote coverage when practical
9. run the full validation gate
10. if asked to commit, create behavior-specific atomic commits

If the user specifies an implementation, validation, or commit order, follow that order exactly instead of batching the feature first.

## Verifying work

For Rust changes, start with the smallest targeted tests that exercise the change, then run the full quality gate before declaring the work complete.

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
```

The project uses strict Clippy settings. Run Clippy early enough that lint-driven design changes do not pile up at the end.

If the task specifically changes feature-gated remote behavior, the remote suite is:

```sh
cargo test --workspace --features DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER
```

Do not run it casually. It is intentionally side-effecting and only appropriate when remote verification is explicitly warranted.

## Git discipline

Treat commits as part of the implementation loop, not as a final cleanup step. While working on a task, commit each coherent change as soon as it is complete and validated, then continue with the next change. Work as a developer would on a WIP PR: the branch should accumulate a sequence of small, understandable commits that reflect the actual progression of the work.

Do not wait until the entire task is finished and bundle everything into one commit. Do not leave multiple independently meaningful completed changes uncommitted while moving on to later work.

For each change:

1. partition work by externally observable behavior
2. separate dependencies, implementation, ignore rules, refactors, and unrelated fixes when independently meaningful
3. split to individual hunks when needed
4. validate the completed change before committing it
5. before every commit, inspect:

```sh
git diff --cached --stat
git diff --cached
git diff --cached --check
```

6. make sure the commit can be understood and reverted independently

Do not make one broad commit simply because all changes belong to the same task.

If an existing commit is too broad, prefer a soft undo and repartition:

```sh
git reset --soft HEAD^
```

Generated verification or rollback artifacts under `.codex/` stay untracked/ignored unless the user explicitly asks to commit them.

Never push or rewrite published history unless the user explicitly asks.

## When instructions disagree

Use this order of precedence:

1. the user's current explicit instruction
2. current repository behavior/tests and explicit project documentation
3. this `AGENTS.md`
4. historical memories

If the undocumented live API contradicts the remembered contract, report the discrepancy. Only update behavior/tests in line with the compatibility goal the user actually requested.
