# Streamable collections

Implementation status: **Implemented in `streamable-rs`.**

Verified against the authenticated web application and current browser bundles
on 2026-08-13. This memory is limited to **My Account > Collections** and its
create, list, view, share, edit, sort, manage, and delete subfeatures.

## Availability and entry points

- The authenticated dashboard exposes **My Account > Collections**, which opens
  the Next.js page at `/collections`.
- Three observed account flags related to this feature were enabled:
  `expose-collections-menu-link`, `collections-enable-edit-mode`, and
  `collections-enable-sort-videos-modal`. The service controls these flags, so
  clients must not assume that every account receives every collection UI.
- The collection index is server rendered. Its document request is
  `GET /collections`; the observed response was HTTP 200 with
  `Cache-Control: no-cache, no-store, must-revalidate`.
- The linked help article, **How to create and share a collection**, currently
  leads to a missing Zendesk article. Treat the inspected application and live
  wire behavior as the usable reference.

## Authentication and common request behavior

- Collection API requests use the authenticated browser session with
  `credentials: "include"`.
- The live API path prefix is `/api/v1`. Browser requests observed the deployed
  API host selected by the application; client implementations should keep the
  host configurable and preserve the path contract below.
- Requests send `Content-Type: application/json`, `Pragma: no-cache`, and
  `Cache-Control: no-cache`. JSON request fields whose values are `undefined`
  are omitted by `JSON.stringify`.
- The current frontend helper parses a response as JSON when possible and as text
  otherwise. A non-success status raises an error containing the service message
  and status. A 204 is treated as a null payload, although the live collection
  delete described below returned 200 with an empty body.

## Creating a collection

- Collection creation starts from the dashboard video list rather than the
  collection index. Select more than one video, then use the header **Share**
  action.
- With only one selected video, Share remains disabled and the tooltip says
  **Select another video to share**. While creation is pending, the action shows
  a loading spinner.
- Wire request:

    ```http
    POST /api/v1/collections
    Content-Type: application/json

    {"shortcodes":["<video-shortcode-a>","<video-shortcode-b>"]}
    ```

- The dashboard model also allows an optional `title` in the create payload, but
  the inspected Share flow sent only `shortcodes`.
- The live service returned HTTP 201 and:

    ```json
    {
        "shortcode": "<collection-shortcode>",
        "title": null,
        "videos": [
            {
                "shortcode": "<video-shortcode-a>",
                "title": "<title-a>",
                "plays": 0
            },
            {
                "shortcode": "<video-shortcode-b>",
                "title": "<title-b>",
                "plays": 0
            }
        ]
    }
    ```

- The response preserved the requested video order. On success, the dashboard
  opens `/c/<collection-shortcode>` in a new tab. On failure, it logs the error
  and shows the server error in a toast.

## Counting and listing collections

- The server-rendered index obtains the count from:

    ```http
    GET /api/v1/collections/count
    ```

    The live response was HTTP 200 with `{"count":1}` while the fixture existed
    and `{"count":0}` after deletion.

- When the server-provided total is zero, `/collections` renders:
  **No collections have been shared yet**. It does not issue a browser list XHR
  for that empty initial state.
- With a nonzero total, the client requests pages of 20:

    ```http
    GET /api/v1/collections?page=1&count=20
    ```

- The live response was HTTP 200 and had no separate total field:

    ```json
    {
        "collections": [
            {
                "shortcode": "<collection-shortcode>",
                "title": "<collection-title>",
                "created_at": "<ISO-8601 timestamp>",
                "updated_at": "<ISO-8601 timestamp>",
                "thumbnail_url": "<signed thumbnail URL>"
            }
        ]
    }
    ```

- The index uses infinite loading for subsequent pages. A row shows its
  thumbnail, title (fallback **Untitled collection**), public `/c/<shortcode>`
  URL, and relative creation time.
- The row menu offers **Edit**, **Manage**, and **Delete**, opening
  `/c/<shortcode>?action=edit`, `?action=manage`, or `?action=delete` in a new
  tab. The adjacent copy button is the list-page **Get link** action.
- A list fetch failure clears loading without a user-facing toast in the current
  reducer/saga path.

## Fetching and viewing a collection

- A public collection page fetches:

    ```http
    GET /api/v1/collections/<collection-shortcode>
    ```

- The authenticated owner-view response observed at HTTP 200 was:

    ```json
    {
        "shortcode": "<collection-shortcode>",
        "title": null,
        "is_owner": true,
        "white_label": false,
        "show_streamable_brand": true,
        "videos": [
            {
                "shortcode": "<video-shortcode>",
                "title": "<video-title>",
                "plays": 0,
                "date_added": "<ISO-8601 timestamp>"
            }
        ]
    }
    ```

- The page subsequently obtains each embedded player's data from
  `GET /api/v1/videos/<video-shortcode>/player`. The inspected render issued two
  player requests per video; do not rely on that duplicate as a protocol
  requirement.
- Owner view exposes **Edit** and **Get link**. The content area shows the
  collection title, playable video cards, each video's title and view count, an
  ad/upgrade area, and the collection footer.
- The current collection fetch saga swallows a failed detail fetch rather than
  mapping it to a visible collection-specific error state.

## Sharing links

- The collection-page **Get link** action writes:
  `/c/<collection-shortcode>?src_collection=copy_link` to the clipboard.
- The list-page copy action writes:
  `/c/<collection-shortcode>?src_collection=copy_link_from_list`.
- In both locations, the success toast says **Link has been copied** and displays
  the clean base `/c/<collection-shortcode>` URL without the analytics query.
- Copying is a local clipboard action; it does not mutate the collection.

## Edit mode and title changes

- **Edit** changes the URL locally to `?action=edit`; it does not require a new
  document navigation. Edit mode exposes **Add video**, **Sort**, **Manage**, and
  **Done**.
- The inline title field autosaves after a 1,000 ms debounce:

    ```http
    PATCH /api/v1/collections/<collection-shortcode>
    Content-Type: application/json

    {"title":"<new title>"}
    ```

- The live PATCH returned HTTP 200 with the collection `shortcode`, updated
  `title`, and ordered video summaries containing `shortcode`, `title`, and
  `plays`.
- Update behavior is optimistic. A failed update restores the previous
  collection snapshot and shows an update failure toast.

## Adding and searching for videos

- **Add video** opens **Add videos to your collection** and loads candidates in
  pages of 12:

    ```http
    GET /api/v1/videos?page=1&count=12&excludeShortcodes=<comma-separated-current-shortcodes>&search=<query>
    ```

- The query is URL encoded. The empty query is sent as `search=`. Current
  collection shortcodes are excluded so already-added videos do not appear.
- With no candidates, the modal shows **No videos found** and disables
  **Add selected videos**. Candidate-load failures show
  **Unable to load videos. Please try again**.
- The search field is debounced by 1,000 ms. The modal supports selecting one or
  more candidates and reports the selected count.
- Adding sends the complete ordered membership, not only the new shortcodes:

    ```http
    PATCH /api/v1/collections/<collection-shortcode>
    Content-Type: application/json

    {"shortcodes":["<existing-shortcode>","<added-shortcode>"]}
    ```

- The live service returned HTTP 200, preserved that order, and the page fetched
  `/videos/<added-shortcode>/player` after the update.
- The frontend caps a collection at 50 videos. Its selector and user-facing
  limit message enforce the cap before add submission.

## Sorting, reordering, and removing videos

- **Sort** opens **Sort videos** with draggable/keyboard rows. Each row's menu
  has **Move up**, **Move down**, and **Remove**.
- Move operations optimistically reorder the local list, wait about 300 ms, then
  PATCH the entire ordered shortcode array. A live move-up changed the wire order
  exactly as shown in the request and the HTTP 200 response preserved it.
- Remove is not a separate endpoint. It marks/removes the local row, waits about
  700 ms, then PATCHes the complete remaining shortcode array:

    ```json
    { "shortcodes": ["<remaining-shortcode>"] }
    ```

- When only one video exists, local state removes it immediately; with multiple
  videos, the row is marked deleted until the request succeeds. A failure
  restores the prior row/state and shows an update failure toast.

## Manage and deep-linked actions

- `?action=manage` opens **Manage collection** with a Title field,
  **Delete collection**, **Cancel**, and **Confirm**. The title can be changed
  here through the same collection update contract.
- Choosing **Delete collection** changes the local action to `?action=delete` and
  opens **Delete collection?**. Directly loading any of the three action query
  values (`edit`, `manage`, or `delete`) restores the corresponding owner UI.
- Cancelling Delete returns to Manage. Cancelling Manage returns to Edit. These
  transitions do not mutate the collection.

## Deleting a collection

- The confirmation explains that the named collection will be deleted forever
  and cannot be restored. The confirm action sends:

    ```http
    DELETE /api/v1/collections/<collection-shortcode>
    ```

- Live deletion returned HTTP 200 with an empty body and redirected the owner to
  the dashboard. A subsequent detail GET returned HTTP 404 with
  `{"statusCode":404,"error":"Not Found","message":"Not Found"}`.
- After deletion, count returned `{"count":0}` and the list returned
  `{"collections":[]}`. Deleting a collection did not delete its member videos.
- The modal stays loading and cannot be closed while deletion is pending. A
  failure clears loading, closes the delete state, and shows a failure toast.

## Parity and future Rust implementation notes

- The sibling Python project has no collection API implementation. Its unrelated
  use of “collection” describes the labels model and is not a parity reference
  for this feature.
- For a Rust client, model collection summary and collection detail responses
  separately: list summaries include timestamps and a signed thumbnail, while
  detail includes ownership/branding fields and dated video entries. Create and
  PATCH responses are smaller collection snapshots.
- Preserve ordered `shortcodes` in create and update models. Title-only PATCHes
  must omit `shortcodes`; membership PATCHes must omit `title` and replace the
  full ordered membership.
- Deterministic tests should assert methods, full `/api/v1` paths, query strings,
  JSON field omission, ordered arrays, empty-body DELETE success, and status/error
  mapping. Any live coverage must remain behind
  `DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER`, use `REMOTE_TEST_LOCK`, make one
  collection from bounded fixtures, delete it as the deletion behavior under
  test, and retain the member videos without fallback cleanup DELETE requests.

## Live verification cleanup

- Inspection used two temporary remote-URL video fixtures and disposable
  collections.
- Both collections were deleted. Count and list were verified empty afterward.
- Both temporary videos were then deleted; each video DELETE returned HTTP 200
  with literal body `true`, and a final title-filtered list found zero remaining
  fixtures.
