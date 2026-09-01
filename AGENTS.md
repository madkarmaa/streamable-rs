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

## A note from the maintainer

I prefer evidence over intuition, small explicit APIs over clever machinery, and designs whose correct behavior is unsurprising. The service is undocumented, so simplicity means making observed constraints visible rather than hiding them behind broad abstractions.

Fight scope creep, but do not compress a change into the wrong layer merely to keep the diff small. Reuse an existing owner when it fits; introduce a new abstraction only when it makes the protocol or lifecycle clearer.

## Before you start

The repository uses a website dump as working context under `dump/`.
Run `bun run scripts/dump.js` before normal project work.

## Browser work

When a task requires Chrome, a live page, browser inspection, reproduced web behavior, or developer tooling:

1. Use the runtime's built-in browser-control API first.
2. Fall back to the `chrome-devtools` MCP only when the built-in path fails, is unavailable, cannot attach to the required page/session, or lacks a required capability.
3. Do not skip directly to the MCP unless the user explicitly asks for it or the task is known to require a DevTools-only capability.
4. After a partial browser action, inspect the current state before retrying. Do not accidentally duplicate submissions, uploads, mutations, account changes, or other side effects.

## Documentation style

Keep user-facing README and rustdoc text short and plain. Remove repeated setup, jargon, and background that does not help the caller use the API.

Give every public module, type, and callable item a small usage example. Keep field and variant descriptions to one clear sentence, and use the parent type's example to show how those parts fit together.

## Debug without leaking secrets

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

## Remote tests touch the real service

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

Unless the user gives a different order, build new API behavior in this sequence:

1. **Observe the wire.** Identify the endpoint, method, payload, aliases, authentication requirement, success shape, and status/error behavior.
2. **Design the boundary.** Keep fixed protocol details inside the client instead of leaking them into the public Rust API.
3. **Model the behavior.** Add request, response, and error models. Preserve established endpoint-specific domain errors and make new failure modes explicit.
4. **Protect the contract.** Add deterministic mock coverage for meaningful wire behavior and mapped statuses before implementing the smallest change at the owning layer.
5. **Validate in layers.** Run targeted tests and Clippy, add bounded feature-gated remote evidence when practical, then run the full validation gate.

If the user specifies an implementation, validation, or commit sequence, follow it exactly.

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

## Let commits tell the story

- **Commit during the implementation loop.** The branch should read like a WIP PR: land each completed, validated behavior before moving to the next independent concern.
- **Keep one concern per commit.** Separate dependencies, implementation, ignore rules, refactors, and unrelated fixes whenever they can be understood and reverted independently; split individual hunks when that is the clearest boundary.
- **Inspect exactly what is staged.** Before every commit, run:

```sh
git diff --cached --stat
git diff --cached
git diff --cached --check
```

- **Repartition broad commits.** For unpublished work, prefer a soft undo:

```sh
git reset --soft HEAD^
```

- **Keep work artifacts out of history.** Generated verification or rollback artifacts under `.codex/` stay untracked or ignored unless the user explicitly asks to commit them.
- **Protect published history.** Never push or rewrite published history unless the user explicitly asks.

## How it works

`StreamableClient<State, T>` combines authentication typestate with a runtime-neutral `HttpTransport`. Request models implement the crate-private `ApiRequest` contract; the client resolves routing, serializes the request, manages cookies, sends it through the transport, and maps the response into endpoint-specific models and errors.

Uploads have an explicit lifecycle. Shortcode allocation returns a `VideoUpload`; completion initializes metadata, streams the file to S3, and requests transcoding. A retained `VideoUploadHandle` keeps remote cancellation available when a runtime drops the completion future.

The CLI is a thin consumer of `core/`. Protocol behavior belongs in the library rather than being reimplemented in `cli/`.

## Where code lives

- `core/src/client/` - typestate client, shared request pipeline, public operations, and client tests.
- `core/src/models/` - public data models plus exact request/response wire models.
- `core/src/transport/` - runtime-neutral HTTP types and the optional reqwest transport.
- `core/src/response/` - shared response decoding.
- `core/src/utils/` - file inspection and S3 signing.
- `core/src/errors.rs` - domain and transport failures.
- `cli/src/main.rs` - the small command-line consumer.
- `.agents/memories/` - durable verified project knowledge, not task notes.

## When instructions disagree

Use this order of precedence:

1. the user's current explicit instruction
2. current repository behavior/tests and explicit project documentation
3. this `AGENTS.md`
4. historical memories

If the undocumented live API contradicts the remembered contract, report the discrepancy. Only update behavior/tests in line with the compatibility goal the user actually requested.

<!-- codebase-memory-mcp:start -->

# Codebase Memory

## Codebase Knowledge Graph (codebase-memory-mcp)

This project uses codebase-memory-mcp to maintain a knowledge graph of the codebase.
ALWAYS prefer MCP graph tools over grep/glob/file-search for code discovery.

### Priority Order

1. `search_graph` — find functions, classes, routes, variables by pattern
2. `trace_path` — trace who calls a function or what it calls
3. `get_code_snippet` — read specific function/class source code
4. `check_index_coverage` — validate candidate paths and missed ranges before claims
5. `query_graph` — run Cypher queries for complex patterns
6. `get_architecture` — high-level project summary

### Evidence tiers

- **Scout (Tier 1):** quick positive lookup with few calls and targeted source checks. Mark it provisional; do not make negative or exhaustive claims.
- **Verify (Tier 2, default):** task-directed graph evidence, relevant trace directions, exact snippets for material claims, and relevant pagination.
- **Auditor (Tier 3):** bounded-scope full verification with current generation, complete relevant pagination, both call directions and broader relationships when material, and every limitation disclosed.
- After candidate paths are known in any tier, call `check_index_coverage` once with every evidence path. Add relevant scopes for negative or exhaustive claims. A clean result means no recorded gap, not proof of completeness. For partial, skipped, excluded, stale, pending, or unknown coverage, read/grep the reported ranges or scope before relying on graph results.

### When to fall back to grep/glob

- Searching for string literals, error messages, config values
- Searching non-code files (Dockerfiles, shell scripts, configs)
- When MCP tools return insufficient results

### Examples

- Find a handler: `search_graph(name_pattern=".*OrderHandler.*")`
- Who calls it: `trace_path(function_name="OrderHandler", direction="inbound")`
- Read source: `get_code_snippet(qualified_name="pkg/orders.OrderHandler")`

### Session resets and subagents

- At session start or after compaction, confirm the nearest graph project and generation with `list_projects` or `index_status`, then choose Scout, Verify, or Auditor.
- Before spawning a subagent, query the graph and coverage in the parent. Pass the tier, project, generation/freshness, bounded scope, queries and pagination state, qualified symbols, paths, call-chain findings, coverage evidence with ranges/reasons, source fallback already performed, and unresolved questions in the delegated task context.
- Do not assume subagents inherit MCP access or the parent conversation. If a child lacks MCP tools, it must not call or claim MCP access. It should use the supplied evidence and read/grep exact source, especially every reported missed-coverage range.

<!-- codebase-memory-mcp:end -->
