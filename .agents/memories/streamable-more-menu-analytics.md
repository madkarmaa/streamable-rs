# Streamable dashboard More menu: Analytics

Implementation status: **Implemented in `streamable-rs`.**

Captured 2026-08-13 from current Streamable web UI, current production JavaScript,
and Chrome DevTools network inspection. This file records only the **Analytics**
feature and its subfeatures. Temporary video IDs, session cookies, signed CDN
parameters, account data, and telemetry identifiers are intentionally omitted.

## Entry point and routing

For a ready, non-expired video card, **More > Analytics** selects one of two
routes behind the `shouldExposeNewAnalyticsSummaryPage` feature flag:

- enabled: `/videos/<shortcode>/analytics`;
- disabled: `/stats/<shortcode>`.

The current live guest session used the enabled route. The Streamable page kept
the dashboard shell at `/videos/<shortcode>/analytics` and rendered an iframe
whose source was:

```text
GET https://dashboard.streamable.com/videos/<shortcode>/analytics
```

Observed response: HTTP 200 HTML, powered by Next.js. The request included the
normal `.streamable.com` session cookie because the iframe is same-site.

## Current analytics summary flow

Current production Next.js code defines these API paths under
`https://api-f.streamable.com/api/v1`:

```text
GET /videos/<shortcode>/analytics?
GET /videos/<shortcode>/analytics/live
```

The first request loads the summary dataset. Production code may append
`MOCK=true` only when its internal mock option is enabled:

```text
GET /videos/<shortcode>/analytics?MOCK=true
```

The summary page is server-rendered. Consequently, its summary API request can
occur on the Next.js server and may not appear as a browser XHR/fetch request.
The iframe document request is browser-visible.

The live-view counter is client-side. After page mount it repeatedly calls:

```text
GET https://api-f.streamable.com/api/v1/videos/<shortcode>/analytics/live
```

On success, the response shape is:

```json
{ "count": 0 }
```

The client updates the displayed count, waits 5,000 ms, and polls again. Any
request failure stops polling rather than retrying indefinitely.

An authenticated production capture confirmed this successful summary shape
through the API's internal `MOCK=true` option:

```json
{
    "countries": [{ "source": "US", "count": 1 }],
    "platforms": [{ "source": "desktop", "count": 1 }],
    "referrers": [{ "source": "direct", "count": 1 }],
    "group": "day",
    "plays": [{ "date": "2026-08-13", "count": 1 }],
    "from_date": "2026-08-13",
    "to_date": "2026-08-14"
}
```

The values vary. The top-level fields and item fields above are the verified
wire names.

## Analytics subfeatures and response concepts

The current summary UI consumes these conceptual groups:

- time-series views, rendered as a line/area chart;
- live views, rendered as a single count and refreshed every five seconds;
- traffic sources, rendered as source/count bars;
- viewer countries/regions, rendered as region/count bars;
- geographic distribution, rendered on a world map;
- device distribution, rendered as source/count pie segments.

The chart adapters expect list items shaped like `{source, count}` for traffic,
region, and device groups. Time-series entries use `{date, count}`. The world-map
adapter converts country-like source codes into `{id, value}`. Empty arrays show
the localized no-data state instead of an empty chart.

## Plan gate

Current code contains an analytics paywall. It fetches current-user state and
checks the account plan. When the plan does not include analytics, the lower
section is blurred and the page offers either a free trial or upgrade. The
feature also links to Streamable's Analytics help article.

## Live guest result

For the disposable guest-owned video used during inspection:

```text
GET /api/v1/videos/<shortcode>/analytics?       -> HTTP 500
GET /api/v1/videos/<shortcode>/analytics/live   -> HTTP 200 {"count":0}
```

The summary error body was:

```json
{
    "statusCode": 500,
    "error": "Internal Server Error",
    "message": "An internal server error occurred"
}
```

The iframe displayed `Error while loading analytics data`. This is a live
guest-account observation rather than a stable contract for authenticated or
paid accounts. The live counter endpoint still succeeded for the same guest
video.

## Legacy analytics route

When `shouldExposeNewAnalyticsSummaryPage` is false, the dashboard navigates to
`/stats/<shortcode>`. Current production source shows the legacy page using the
old host `https://ajax.streamable.com` and these requests:

```text
GET /<shortcode>/plays
GET /<shortcode>/articles
GET /<shortcode>/stats/live
```

The first two populate historical plays and referring articles. The live stats
request populates the legacy live view state. These endpoints are source-derived
fallback behavior; the current live session selected the new summary branch.

## Rust API mapping

`StreamableClient` exposes separate bounded calls:

- `get_video_analytics` returns `VideoAnalyticsSummary`;
- `get_video_live_views` returns `VideoLiveViews`.

Both work for authenticated and unauthenticated client states. They keep route
details internal, send bodyless GET requests, and preserve the status and server
message in endpoint-specific errors. There is no background polling helper;
callers choose if and when to repeat the live call.

## Coverage

Deterministic tests cover bodyless GETs on both analytics paths, zero live-view
counts, and endpoint-specific errors that preserve status and server message.
The feature-gated remote test uploads one small video, leaves it in the shared
test account, and performs one live-view GET without relying on the guest HTTP
500 remaining stable.
