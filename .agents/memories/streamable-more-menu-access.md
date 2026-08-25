# Streamable More-menu client access

Request testing established that every video feature represented by the
More-menu memories is available through both authenticated and unauthenticated
client states except Captions and Edit labels. Captions and all label operations
require `StreamableClient<Authenticated>`.

The remaining video-scoped operations live on `StreamableClient<State>`.
Unauthenticated access does not discard anonymous-session cookies already held
by the shared client. Account-scoped CRUD and account settings remain on the
authenticated client even when a related video feature reads account metadata.
Label CRUD and `set_video_labels` are authenticated-only, matching the verified
wire behavior and deterministic client coverage.
