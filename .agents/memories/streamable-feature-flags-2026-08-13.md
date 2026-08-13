# Streamable authenticated feature flags

Verified against the authenticated web dashboard on 2026-08-13. This memory is
limited to the feature-flag fetch, consumption, and local override workflow.

## Wire and dashboard behavior

- The dashboard fetches `GET /api/v1/me/flags` with the authenticated session by
  using `credentials: "include"`.
- A successful response is an object shaped as `{ "flags": { ... } }`.
- The current-user fetch flow requests flags both during normal bootstrap and
  after login/registration, then merges the returned `flags` object into the
  current-user state.
- Flag values are typed configuration, not only booleans. The observed response
  contained booleans plus strings, a number, and an object.
- The live list is service-controlled and can drift. Consumers should enumerate
  the keys returned by the API rather than maintain a supposedly complete static
  list.

## Observed list shape

- The observed response had 25 keys: 19 boolean and 6 non-boolean values.
- Eight booleans were disabled for the disposable account used for inspection.
- Examples of non-boolean configuration include a checkout-plan string, an API
  version number, date-like rollout thresholds, and an object-valued dashboard
  banner configuration.
- A disabled UI flag can hide a dashboard entry while its underlying endpoint
  remains independently governed by server behavior.

## Local override userscript

- `scripts/streamable-feature-flags.user.js` installs at `document-start` on the
  Streamable dashboard and wraps `window.fetch` before application bootstrap.
- It only intercepts the exact `api-f.streamable.com/api/v1/me/flags` request. It
  leaves all other requests unchanged and never writes flag values to the API.
- The wrapper clones and parses the server response, retains every server-returned
  key, applies stored overrides only to keys present in that response, and returns
  the patched envelope to the dashboard.
- Overrides are stored in browser `localStorage` under
  `streamable-feature-flag-overrides-v1`; the server values remain visible beside
  the effective local values.
- The injected `FF` panel fetches the full current API list, supports search,
  provides Server/On/Off choices for booleans, accepts JSON values for other
  types, clears individual or all overrides, and reloads the page to apply changes
  before the dashboard consumes the response.
- The console controller is `window.StreamableFeatureFlags`, with `refresh`,
  `list`, `get`, `set`, `unset`, `clear`, `open`, and `close` operations.

## Verification notes

- The live endpoint returned HTTP 200 and all 25 current keys without account
  mutation.
- A browser harness exercised the exact userscript with five mixed-type flags. It
  loaded all five, changed one server-false boolean to effective true, preserved
  an object value, removed stale content-length metadata, rendered five rows, and
  restored the server value after clearing overrides.
- Syntax and formatting checks are `node --check` and Prettier respectively.

