# User models

## Shared user totals (2026-08-19)

`UnauthenticatedUser` owns the common `socket`, `total_plays`, `total_uploads`,
and `total_videos` fields. `AuthenticatedUser` retains that value in its public
`unauthenticated` field and implements `Deref<Target = UnauthenticatedUser>`.
Authenticated callers can therefore use `client.user().total_videos` while both
user model types and the explicit `user.unauthenticated` access path remain
available. The API response exposes both `total_uploads` and `total_videos`;
they remain distinct fields rather than aliases.
