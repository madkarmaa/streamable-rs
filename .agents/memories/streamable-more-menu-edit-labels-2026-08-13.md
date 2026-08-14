# Streamable dashboard More menu: Edit labels

Implementation status: **Implemented in `streamable-rs` through
`AuthenticatedStreamableClient::set_video_labels`.**

Captured 2026-08-13 from the authenticated Streamable dashboard, production
source maps, and Chrome DevTools network inspection. This file records only the
**Edit labels** feature and its label-creation, assignment, removal, bulk-state,
error, and cleanup behavior. Disposable account details, session cookies,
video shortcodes, label IDs, and telemetry identifiers are intentionally
omitted.

## Authenticated availability

The feature requires an authenticated dashboard session. The live account
returned HTTP 200 from both `/api/v1/me` and `/api/v1/me/flags`. Its relevant
feature flags were:

```json
{
    "is-labels-enabled": true,
    "labels-sidebar-unlabelled-videos-view": true
}
```

The dashboard loads the label catalog with:

```http
GET https://api-f.streamable.com/api/v1/labels
Cookie: <session; redacted>
```

The response envelope is:

```json
{
    "userLabels": [{ "id": "<label-id>", "name": "<label-name>" }]
}
```

The video card's **More > Edit labels** item is conditional on the card
receiving an edit-labels handler. In the inspected dashboard, that handler was
present after the account had at least one label and absent after every label
was deleted. Thus authenticated state and an enabled labels flag are not alone
sufficient to make the menu item visible; an empty label catalog currently
removes it.

The same modal can also be opened from the label pill on the video card. When
the account has labels, an unlabeled card shows a `None` label pill.

## Modal and local selection behavior

Both entry points open a modal titled `Labels` containing:

- a `Search labels` input;
- one checkbox per account label;
- `Cancel` and `Save` actions;
- an inline create-label action when the search has no matches.

Search performs a case-insensitive substring match against label names. It does
not change the underlying selection state. When the filtered list is empty and
the search value is nonblank after trimming, the modal shows:

```text
Create new label "<search value>"
```

Each selection entry carries three meaningful fields:

```text
id, checked, indeterminate
```

For a single video, `checked` mirrors whether the video currently has the
label. For a bulk selection, `indeterminate` means only some selected videos
have it. The first user toggle of an indeterminate entry clears
`indeterminate` and checks the entry; later toggles invert `checked` normally.

Accessible checkbox labels reveal the intended operations:

- unchecked: `Apply <name>`;
- checked: `Remove <name> label`;
- indeterminate: `Apply <name> from some to all`.

The modal disables `Save` while assignments are loading or when the entire
selection-state array is empty. It does not require any checkbox to remain
checked: when labels exist, saving an all-unchecked state is how all labels are
removed. Label checkboxes are disabled while saving or creating a label.

Closing or cancelling clears the modal's local selection without making an API
request.

## Create a label inside the modal

The inline create action sends:

```http
POST https://api-f.streamable.com/api/v1/labels
Content-Type: application/json
Pragma: no-cache
Cache-Control: no-cache
Cookie: <session; redacted>

{"name":"<search value>"}
```

The observed response was HTTP 201:

```json
{ "name": "<label-name>", "id": "<label-id>" }
```

Important current UI details:

- the request uses the current search string, not the later trimmed value;
- after dispatching the request, the UI trims the search input so a newly
  created label remains visible;
- the client augments the response with local `count: 0`;
- the new label is merged into the selection as `checked: true` and
  `indeterminate: false`;
- creation alone does not assign the label to the video; the user must still
  save the modal.

For an HTTP 4xx response, the client reads the JSON `message` and displays it.
For other non-success statuses it displays
`Something went wrong, try again in a few minutes.` A thrown network error uses
its message when available, otherwise the same generic message.

## Save the complete label assignment

Saving sends one request for each selected video:

```http
POST https://api-f.streamable.com/api/v1/videos/<shortcode>/labels
Content-Type: application/json
Pragma: no-cache
Cache-Control: no-cache
Cookie: <session; redacted>

{"labels":[<label-id>,<label-id>]}
```

The body is an absolute replacement list, not an add/remove command. Label IDs
are sorted numerically before serialization to make requests deterministic.

For each video, the dashboard computes the final list as follows:

1. labels still marked `indeterminate` were not changed and retain their
   existing per-video state;
2. non-indeterminate checked labels are included;
3. non-indeterminate unchecked labels are excluded;
4. the final list is checked IDs plus existing IDs that remain indeterminate;
5. add/remove ID deltas are computed separately only for updating Redux state.

The live single-video assignment sent a one-ID `labels` array. Removing it sent
exactly:

```json
{ "labels": [] }
```

Both calls returned HTTP 200 with a zero-length response. The client tests only
`response.ok` and does not parse a response body. Chrome reported
`net::ERR_ABORTED` after the successful bodyless response; this is not treated
as an application failure, and the card label updated correctly.

For multiple selected videos, the frontend starts all per-shortcode POSTs with
`Promise.all`. Each success updates the local video's labels and count deltas;
the update carries a partial-fulfilment marker when more than one shortcode is
in the operation. If every response succeeds, the modal closes and the
dashboard deselects all videos.

If any response is not successful, the client collects failed shortcodes and
shows:

```text
The videos <comma-separated-shortcodes> failed to assign the labels, please try again in a few minutes.
```

An unexpected outer failure shows:

```text
Something went wrong, try again in a few minutes.
```

The current code does not parse a server error body for assignment failures and
does not roll back requests that already succeeded in a partially failed bulk
operation.

## Live authenticated verification and cleanup

A disposable account was used to avoid relying on guest-only UI behavior. The
verified sequence was:

1. an invalid login request returned HTTP 200 with `error: "AuthError"` and
   `message: "Invalid username or password"`;
2. a disposable account registration and subsequent credential login each
   returned HTTP 200 and an authenticated user representation;
3. creating the account's first label returned HTTP 201 and made **Edit
   labels** appear in the More menu;
4. assigning that label returned HTTP 200 with an empty response and changed
   the card pill from `None` to the label name;
5. saving `{"labels":[]}` returned HTTP 200 and restored the card pill to
   `None`;
6. searching for a missing label inside the modal exposed the create action;
   creation returned HTTP 201, auto-checked the new label, and a later Save
   assigned it;
7. the video was restored to an empty labels array;
8. both disposable labels were deleted with bodyless
   `DELETE /api/v1/labels/<label-id>` requests, each returning HTTP 204;
9. a final GET returned an empty `userLabels` array, and reloading the
   dashboard removed **Edit labels** from the More menu again.

No account identifier, credential, cookie, concrete shortcode, or concrete
label ID is durable project state.

## Rust implementation

The Rust client keeps video assignment separate from account-label CRUD:

```text
set_video_labels(shortcode, label_ids)
```

`SetVideoLabelsRequest` keeps the fixed `/videos/<shortcode>/labels` suffix
internal. The public method accepts a label-ID slice, sorts IDs before
serialization, sends the authenticated session cookie, and accepts a
successful empty response rather than attempting JSON decoding. Passing an
empty slice removes all labels. Other non-success statuses map to
`VideoLabelAssignmentFailed`, while shared invalid-session and rate-limit
responses retain their existing domain errors.

Deterministic mock tests cover the exact path, method, cookie, sorted and empty
replacement bodies, bodyless success, endpoint-specific failure, and shared
error mappings. A bounded feature-gated remote test uploads one disposable
video, creates one disposable label, assigns and clears it, then deletes both
resources. It is compiled by the all-features Clippy gate but is not run by
default.

Bulk assignment can be built above the single-video primitive. If it mirrors
the dashboard, document that it is non-transactional: successful videos remain
changed when another video's request fails. A Rust bulk API should return
per-shortcode results rather than collapsing partial failure into a generic
error string.

Do not infer menu visibility solely from authentication. The web UI also
depends on feature state and whether at least one label exists.

## Deterministic test targets

Local mock coverage should verify:

1. method and effective path are exactly POST and
   `/api/v1/videos/<shortcode>/labels`;
2. the session cookie is present;
3. the JSON body contains only `labels`;
4. IDs are sorted before serialization;
5. an empty slice serializes as `{"labels":[]}`;
6. HTTP 200 with an empty body succeeds;
7. a non-success response maps to an explicit assignment error without
   requiring a JSON body;
8. create-label remains a separate POST to `/api/v1/labels` with a `name`
   field;
9. a bulk helper preserves per-video success/failure results and makes no
   rollback claim.

Any remote coverage must remain behind
`DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER`, use a disposable authenticated
account and video, serialize mutations through `REMOTE_TEST_LOCK`, restore the
video's original labels, and delete created labels when cleanup is supported.
