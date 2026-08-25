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

## Maintainer taste

Prefer evidence over intuition, small explicit APIs over clever machinery, and designs whose correct behavior is unsurprising. The service is undocumented, so simplicity means making observed constraints visible rather than hiding them behind broad abstractions.

Fight scope creep, but do not compress a change into the wrong layer merely to keep the diff small. Reuse an existing owner when it fits; introduce a new abstraction only when it makes the protocol or lifecycle clearer.

Most of this document describes strong defaults. The user's current explicit direction can override process, order, and scope. Rules about live requests, credentials, destructive actions, and published history are hard safety boundaries: crossing them requires explicit authorization rather than inference.

## Before you start

The repository uses a generated repository dump as working context.

If the repository-root `dump/` directory does not exist, run the repository-root `dump.py` before normal project work. Do not regenerate an existing dump merely because it exists unless the current task actually requires a fresh one.

## How it works

`StreamableClient<State, T>` combines authentication typestate with a runtime-neutral `HttpTransport`. Request models implement the crate-private `ApiRequest` contract; the client resolves routing, serializes the request, manages cookies, sends it through the transport, and maps the response into endpoint-specific models and errors.

Uploads have an explicit lifecycle. Shortcode allocation returns a `VideoUpload`; completion initializes metadata, streams the file to S3, and requests transcoding. A retained `VideoUploadHandle` keeps remote cancellation available when a runtime drops the completion future.

The CLI is a thin consumer of `core/`. Protocol behavior belongs in the library rather than being reimplemented in `cli/`.

## Where code lives

- `core/src/client/` — typestate client, shared request pipeline, public operations, and client tests.
- `core/src/models/` — public data models plus exact request/response wire models.
- `core/src/transport/` — runtime-neutral HTTP types and the optional reqwest transport.
- `core/src/response/` — shared response decoding.
- `core/src/utils/` — file inspection and S3 signing.
- `core/src/errors.rs` — domain and transport failures.
- `cli/src/main.rs` — the small command-line consumer.
- `.agents/memories/` — durable verified project knowledge, not task notes.

## Browser work

When a task requires Chrome, a live page, browser inspection, reproduced web behavior, or developer tooling:

1. Use the runtime's built-in browser-control API first.
2. Fall back to the `chrome-devtools` MCP only when the built-in path fails, is unavailable, cannot attach to the required page/session, or lacks a required capability.
3. Do not skip directly to the MCP unless the user explicitly asks for it or the task is known to require a DevTools-only capability.
4. After a partial browser action, inspect the current state before retrying. Do not accidentally duplicate submissions, uploads, mutations, account changes, or other side effects.

## Documentation style

Keep user-facing README and rustdoc text short and plain. Remove repeated setup, jargon, and background that does not help the caller use the API.

Give every public module, type, and callable item a small usage example. Keep field and variant descriptions to one clear sentence, and use the parent type's example to show how those parts fit together.

## Hard boundary: debugging and sensitive values

Concentrate debug instrumentation at the existing shared request pipeline, response decoder, transport, file-inspection, upload, signing, and rollback seams instead of duplicating it in each endpoint method.

Never record raw request or response bodies, header or cookie values, passwords, account details, generated credentials, authorization values, policies, signatures, session or transcoder tokens, signed URLs, or sensitive error text. Applications retain ownership of tracing subscribers and filters.

Treat temporary credentials and response-provided signed URLs as opaque secrets. Do not persist them in tracked fixtures or memories, expose generated naming rules as stable API, or reconstruct a signed URL when the service supplies the authoritative value.

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

## Hard boundary: remote tests

Normal tests must never contact Streamable, create accounts, mutate labels, upload files, or otherwise send requests to the live service.

The explicit opt-in feature is:

```text
DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER
```

Remote tests read `EMAIL` and `PASSWORD` from the repository-root `.env`. Keep only empty placeholders in the tracked `.env.example`; real credentials belong only in the ignored `.env`.

Except for registration tests, use the existing lazily authenticated client and shared account for the test process. Registration tests create users because registration is the behavior under test. Password tests are the only shared-state restoration exception: restore the shared password so later tests can authenticate.

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

## Adding API behavior

Start with the observed wire contract: endpoint, method, payload, aliases, authentication, success shape, and status/error behavior. Fixed protocol details stay inside the client rather than leaking into the public API.

A complete feature normally includes the request, response, and error models; deterministic mock coverage for the meaningful wire behavior and mapped statuses; the smallest implementation at the owning layer; targeted tests and Clippy; bounded feature-gated remote evidence when practical; and the full validation gate before completion.

Preserve established endpoint-specific domain errors and make new failure modes explicit. Broad catch-all errors are a compatibility tool, not a default.

That order is a working default. If the user specifies an implementation, validation, or commit sequence, follow it exactly.

## Verifying

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

## Git and work history

Treat commits as part of the implementation loop. The branch should read like a WIP PR: each completed, validated behavior lands as a small commit before work moves to the next independent concern.

One concern per commit. Separate dependencies, implementation, ignore rules, refactors, and unrelated fixes whenever they can be understood and reverted independently; split individual hunks when that is the clearest boundary.

Before every commit, inspect exactly what is staged:

```sh
git diff --cached --stat
git diff --cached
git diff --cached --check
```

If an existing commit is too broad, a soft undo is the preferred way to repartition unpublished work:

```sh
git reset --soft HEAD^
```

Generated verification or rollback artifacts under `.codex/` stay untracked or ignored unless the user explicitly asks to commit them. Never push or rewrite published history unless the user explicitly asks.

## When instructions disagree

Use this order of precedence:

1. the user's current explicit instruction
2. current repository behavior/tests and explicit project documentation
3. this `AGENTS.md`
4. historical memories

If the undocumented live API contradicts the remembered contract, report the discrepancy. Only update behavior/tests in line with the compatibility goal the user actually requested.
