# Streamable More-menu client access

User-confirmed client contract: every video feature represented by the
More-menu memories is available through both authenticated and unauthenticated
client states, except Captions and Edit labels. Captions and all label operations
require `StreamableClient<Authenticated>`.

Implement the remaining video-scoped operations on the generic
`StreamableClient<State>` rather than restricting them to
`StreamableClient<Authenticated>`.
Unauthenticated means that account authentication is not required; it does not
mean that requests must discard cookies. The shared client must continue sending
any anonymous-session cookies already present in its cookie jar.

This rule does not move account-scoped CRUD or account settings methods onto the
generic client merely because a related video feature reads account metadata.
Label CRUD and `set_video_labels` are authenticated-only, matching the verified
wire behavior and deterministic client coverage.
