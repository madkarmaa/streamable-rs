# Core HTTP transport

## Runtime-neutral transport migration (2026-08-19)

The earlier note that native reqwest and Tokio define the core runtime contract
is superseded. `core/src/transport/` now owns runtime-neutral `Request`,
`Response`, `Body`, and `HttpTransport` types. `Body::File(PathBuf)` delegates
large-file streaming to each transport. The default `reqwest` feature supplies
`ReqwestTransport`; `--no-default-features` removes reqwest and Tokio from normal
core dependencies, and callers construct clients with `StreamableClient::with_transport`.

`StreamableClient<State, T>` preserves its transport through authentication and
logout typestate changes. The client, not reqwest, owns `cookie_store::CookieStore`,
adds request cookies, and consumes every `Set-Cookie` response header. Protocol
models use `http::Method`, `http::HeaderMap`, and runtime-neutral bodies; no model
uses `reqwest::RequestBuilder`. `ApiResponse` maps non-success responses from its
`http::StatusCode` rather than retaining `reqwest::Error`.

`StreamableClient::logout` is infallible: it consumes the authenticated client,
preserves the existing transport and endpoint routing, clears cookies, and
returns `UnauthenticatedStreamableClient<T>` directly.

`core/src/client/mod.rs` keeps request-specific URL resolution, serialization,
and response decoding in `StreamableClient::execute<Req>`. The async transport
pipeline lives in module-level `send_request<T>`, which is generic only over the
transport and owns cookies, default JSON content type, transport execution, and
response-cookie storage. Keeping it independent of both request and client
typestate avoids duplicating that state machine for every API request or auth
state. For the five CLI request types measured with `cargo llvm-lines --release`,
each `execute` closure fell from 521 to 297 LLVM IR lines and one 459-line shared
`send_request` closure replaced the duplicated pipeline (2,605 to 1,944 lines,
about 25% fewer across those functions).
