# Streamable dashboard More menu: Delete

Implementation status: **Not implemented in `streamable-rs`.**

Verified against the authenticated web dashboard on 2026-08-13. This records the
wire contract and UI behavior for one feature only: deleting a video.

## Entry and confirmation

- A ready video's `More` menu exposes `Delete`.
- Selecting it is local-only and opens a modal titled `Delete Video`; no request is
  sent yet.
- When a title exists, the prompt is `Are you sure you want to delete <title>?`.
  The component falls back to `this video` when no title exists.
- Closing or cancelling the modal sends no request.
- Confirming closes the modal first, then starts deletion.

## Wire contract

```http
DELETE /api/v1/videos/<shortcode>
Cookie: <authenticated session>

<no request body>
```

- The browser uses `credentials: "include"`.
- The request has no JSON payload and does not add a request `Content-Type`.
- The observed success response was HTTP `200`, content type
  `application/json; charset=utf-8`, content length `4`, with the literal response
  body `true`.
- The dashboard reads the response as text and accepts only the exact string
  `true`. HTTP success alone is insufficient: an empty `204`, `false`, or JSON
  string `"true"` follows the failure path.
- After confirmed deletion, `GET /api/v1/videos/<shortcode>` returned HTTP `404`
  with the usual structured Not Found object. The authenticated videos listing
  returned HTTP `200`, total `0`, no entries, and the dashboard showed
  `All videos (0)` after reload.

## Dashboard state and errors

- The thunk first dispatches the video with `isDeleting: true`, but the video
  reducer deliberately retains the card during that intermediate action.
- On exact-literal success, the reducer removes the shortcode from the ordered
  shortcode/upload collections and the video cache. Related label counts are
  decremented from the deleted video's label membership.
- Any thrown fetch error or non-`true` response produces the fixed toast
  `Error deleting video`, logs a diagnostic, and dispatches the error so the card
  remains.
- A pending local upload can be removed from reducer state by `local_id`, but the
  remote delete thunk only sends a request when a `shortcode` exists. Upload
  cancellation is a separate flow.

## Bulk and alternate entry points

- Bulk deletion maps the selected videos to the same delete thunk and awaits
  them with `Promise.all`.
- Each individual thunk catches its own failure. The bulk operation therefore
  deselects everything after all calls settle even when only some deletions
  succeeded; there is no transactional rollback.
- The flagged-video `Dismiss` action calls the same delete thunk without this
  confirmation modal.

## Rust parity guidance

- The inspected sibling Python client has no video-delete method to copy. Treat
  the observed dashboard request as the current parity reference.
- A Rust method should keep the fixed path internal, accept a shortcode, include
  the authenticated session, send a bodyless `DELETE`, and expose a typed failure
  when the response text is not exactly `true`.
- Deterministic mock coverage should assert the full effective path, method,
  bodylessness, authentication behavior, literal `true` success, and failures for
  `false`, `"true"`, empty `204`, transport errors, and ordinary error statuses.
- Client-state tests should separately cover retaining the item while deletion is
  pending, successful cache/list removal, label-count adjustment, failure
  retention, local-id removal, and partial-success bulk behavior.
- Any remote test must remain behind
  `DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER`, use `REMOTE_TEST_LOCK`, create one
  disposable video, issue one delete, and verify the resulting Not Found state.
