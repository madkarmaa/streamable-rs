# Streamable dashboard More menu: Privacy

Captured 2026-08-13 from the authenticated Streamable dashboard, production
source maps, Chrome DevTools network inspection, and comparison with the
sibling Python client. This file records only the **Privacy** feature and its
visibility, restrictions, player-preference, upsell, reset, refresh, and error
branches. Disposable account data, cookies, video shortcodes, plan identifiers,
and telemetry are intentionally omitted.

## Entry, URL state, and initial data

For a ready video, **More > Privacy** does not navigate to a separate page or
immediately call a privacy endpoint. It updates browser history with:

```text
/?modal=video-privacy&shortcode=<shortcode>
```

The dashboard opens `Privacy Settings` from the video already in its Redux
cache. On initial page render, the modal watcher waits for the video-list fetch
to finish, inspects those two query parameters, and opens only if that
shortcode is present in cache. This guard prevents a stale modal from opening
later after unrelated navigation.

Closing the modal removes both `modal` and `shortcode` with `history.pushState`
and returns to `/`; it does not make a network request.

The authenticated account's relevant feature flag was:

```json
{ "expose-video-privacy-change": true }
```

The modal reads these fields from the cached video's `privacy_settings`:

```text
visibility
allow_download
allow_sharing
domain_restrictions
allowed_domain
password_protected
hide_view_count
is_custom
```

UI fallbacks when fields are absent are `public`, `false`, `false`, `off`, an
empty allowed-domain string, no password, visible view count, and non-custom
settings.

## Shared mutation and refresh flow

Every individual setting mutation uses the same endpoint:

```http
PATCH https://api-f.streamable.com/api/v1/videos/<shortcode>/settings
Content-Type: application/json
Pragma: no-cache
Cache-Control: no-cache
Cookie: <session; redacted>

{<only changed field>}
```

The live service returned HTTP 204 with no body. Each successful PATCH was
immediately followed by:

```http
GET https://api-f.streamable.com/api/v1/videos/<shortcode>
Cookie: <session; redacted>
```

The GET returned HTTP 200 with the full video object. The UI relies on that
refetch rather than synthesizing authoritative privacy state from the PATCH.
Chrome can display `net::ERR_ABORTED` for the successful bodyless 204; this is
not an application error.

The shared request helper treats 204 as `null`, parses other responses as JSON,
and throws `payload.message` for a non-success response. Sagas show that message
in a toast and log it. Visibility and toggle failures also dispatch compensating
actions so the optimistic UI returns to the last video value. A missing cached
video produces `Something unexpected occurred` without sending a request.

## Visibility

The modal exposes three wire values:

| UI label           | Wire value             | Description                                      |
| ------------------ | ---------------------- | ------------------------------------------------ |
| Public             | `public`               | Anyone with a link can view.                     |
| Hide on Streamable | `hidden_on_streamable` | Private on the account, but embeddable anywhere. |
| Private            | `private`              | Only the owner can view.                         |

Changing an available option sends only:

```json
{ "visibility": "<wire-value>" }
```

On the free account, **Hide on Streamable** was labeled `BASIC`. Selecting it
did not call the settings endpoint; it opened the plan chooser and added:

```text
checkout=pick-a-plan
from=video-privacy
src_internal=visibility
```

Selecting **Private** was available on the free account. The observed sequence
was:

```text
PATCH /api/v1/videos/<shortcode>/settings  {"visibility":"private"}  -> 204
GET   /api/v1/videos/<shortcode>                                      -> 200
```

The refetched state changed `visibility` to `private` and kept
`allow_sharing:true`; the service did not forcibly clear that stored setting.
While private, the UI disabled Domain Privacy, Allow downloading, Allow
sharing, and the flag-gated view-count control. Returning to Public sent the
same two-request sequence with `{"visibility":"public"}` and re-enabled the
controls.

## Domain Privacy and allowed domains

In per-video layout, Domain Privacy is labeled `PRO`. The plan rules are:

- no active plan: disabled control; click opens the general upgrade modal;
- Basic or Elements plan: individual control remains unavailable and can open
  the change-plan flow;
- sufficient plan: toggle is enabled unless visibility is `private`.

The free-account click made no settings request and opened the plan chooser
with:

```text
checkout=pick-a-plan
from=video-privacy
src_internal=domain-privacy
```

For an eligible plan, source inspection shows that the toggle sends one field:

```json
{ "domain_restrictions": "allowlist" }
```

or:

```json
{ "domain_restrictions": "off" }
```

and then refetches the video.

When the effective value is `allowlist`, the modal exposes an `Allowed domains`
text field. Its current validation is deliberately weak: a value is considered
valid whenever it does not contain `/`. Comma-separated domains are accepted.
An invalid value remains local and shows `Must be a domain like site1.com`.

A valid edit is debounced for 1000 ms and sends:

```json
{ "allowed_domain": "<input string>" }
```

The field shows `Saving...` during the request and, after the refetch,
`Saved. Changes will take effect within the next 30 minutes.` The server error
message is shown in a toast. The input does not perform hostname normalization,
trimming, or slash removal before sending.

## Password protection

Password protection is labeled `BASIC`. With no active plan, clicking the
disabled row made no settings request and opened the plan chooser with:

```text
checkout=pick-a-plan
from=video-privacy
src_internal=password-protection
```

For an eligible plan, turning the toggle on is initially local-only. The modal
shows a `Create password` field and enables `Set password` only when the value
is nonempty. Submission sends:

```json
{ "password": "<new password>" }
```

After the PATCH and video refetch, the UI uses only the returned
`password_protected` boolean. It never retrieves or displays the password. A
protected video instead offers `Change password`, which returns the local UI to
the empty password form. Turning protection off sends:

```json
{ "password": null }
```

followed by the normal video refetch.

## Player preferences

### Allow downloading

The toggle sends exactly:

```json
{ "allow_download": true }
```

or:

```json
{ "allow_download": false }
```

The live account exercised both directions. Each PATCH returned 204 and each
following video GET returned 200. The final value was restored to `false`.

### Allow sharing

The toggle sends exactly:

```json
{ "allow_sharing": false }
```

or:

```json
{ "allow_sharing": true }
```

The live account exercised both directions with the same 204-then-200 flow.
The final value was restored to `true`.

### View count

This row exists only when the `configurable-view-count` flag is true. The live
account's flag was false, so the row was absent. Source inspection shows a
positive UI label, `View count`, whose switch is checked when
`hide_view_count` is false. Toggling sends the underlying inverse field:

```json
{ "hide_view_count": true }
```

or:

```json
{ "hide_view_count": false }
```

Private visibility disables this control when it is exposed.

## Custom-setting indicator and Reset defaults

When `privacy_settings.is_custom` is false, the warning says that the video uses
default privacy settings and links to `/settings`. When it is true, the warning
says `This video has custom privacy settings` and exposes `Reset defaults`.

Reset opens a second modal titled `Confirmation required` with:

```text
You are about to apply default privacy settings to this video.
This action cannot be undone.
```

Confirm sends a bodyless request:

```http
DELETE https://api-f.streamable.com/api/v1/videos/<shortcode>/settings
Content-Type: application/json
Pragma: no-cache
Cache-Control: no-cache
Cookie: <session; redacted>
```

The observed DELETE returned HTTP 204 and was followed by the normal full-video
GET, which returned HTTP 200. The confirmation button was disabled while the
operation ran, and the confirmation modal closed after success. On failure it
stays open, re-enables Reset, and shows the server message in a toast.

After live reset, the verified privacy state was:

```json
{
    "visibility": "public",
    "allow_download": false,
    "allow_sharing": true,
    "domain_restrictions": "off",
    "allowed_domain": "",
    "password_protected": false,
    "hide_view_count": false,
    "is_custom": false
}
```

This is the account's current default state. Reset is the reliable cleanup path
after exercising multiple per-video settings; it removes the override rather
than merely writing values that happen to match defaults.

## Live flow summary

The disposable authenticated video was used for these observed branches:

1. opening and closing the modal changed only URL search parameters;
2. free-plan Hide on Streamable, Domain Privacy, and Password protection clicks
   opened source-specific upgrade flows without a settings mutation;
3. Private and Public each sent a one-field PATCH, received 204, and refetched
   the video with GET 200;
4. Allow sharing was turned off and back on through two one-field PATCHes;
5. Allow downloading was turned on and back off through two one-field PATCHes;
6. Reset defaults sent a bodyless DELETE 204 and a GET 200;
7. final state matched account defaults with `is_custom:false`.

No concrete shortcode, account field, password, cookie, or plan identifier is
durable project state.

## Python parity and future Rust API

The sibling Python implementation exposes account-default privacy changes via
`/me/settings`; it does not currently implement the per-video
`/videos/<shortcode>/settings` PATCH or DELETE flow. The existing Rust
`change_privacy_settings` operation also targets account settings. This More
menu behavior therefore needs separate per-video operations rather than an
extension that silently changes the existing account-level method's endpoint.

A suitable Rust surface is:

```text
update_video_privacy(shortcode, partial_settings)
reset_video_privacy(shortcode)
```

The partial model should omit absent fields and support:

```text
visibility: public | hidden_on_streamable | private
allow_download: bool
allow_sharing: bool
domain_restrictions: off | allowlist
allowed_domain: string
password: string | null
hide_view_count: bool
```

Do not model `password_protected` or `is_custom` as writable fields; they are
read-side state from the refetched video. Treat successful 204 responses as
empty, and keep the follow-up GET explicit if the API should return the updated
video.

## Deterministic test targets

Local mock coverage should verify:

1. each update uses PATCH on the full
   `/api/v1/videos/<shortcode>/settings` path;
2. the authenticated cookie and JSON/cache-control headers are present;
3. a single-field change serializes only that field;
4. `hidden_on_streamable` is preserved as an exact wire value;
5. password removal serializes JSON null;
6. reset uses bodyless DELETE on the same path;
7. HTTP 204 succeeds without JSON decoding;
8. the optional refresh performs GET `/api/v1/videos/<shortcode>` after a
   successful mutation and not after a failed mutation;
9. a non-success JSON `message` maps to an explicit per-video privacy error;
10. account-level `/me/settings` behavior remains unchanged.

Any remote coverage must remain behind
`DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER`, use a disposable authenticated
video, serialize through `REMOTE_TEST_LOCK`, make the minimum mutations, and
finish with DELETE reset plus a GET proving `is_custom:false`.
