# Streamable web: Merge selected videos

Implementation status: **Not implemented in `streamable-rs`.**

## Selection and entry point

- Selecting any dashboard video replaces the normal list header with a selection header and an `<n> items selected` count.
- **Merge** links to `/merge`, but remains visually disabled with `aria-disabled` until more than one video is selected.
- With exactly one selection, its tooltip says `Select another video to merge`.
- With two or more selections, the header exposes **Edit Labels**, **Merge**, **Share**, and **Delete**.
- `/merge` is an SPA route that lazy-loads the joiner. Opening it without selected videos redirects to `/`.
- No explicit maximum selection count was found in the joiner; its verified lower bound is two.

## Merge page behavior

- The preview treats selected videos as one concatenated timeline. It plays clips in selection order, advances to the next clip when one ends, and wraps from the last clip to the first.
- The combined progress bar uses the sum of all clip durations. Scrubbing maps the global position to a particular clip and local playback time.
- Each row shows a thumbnail, title with `Untitled` fallback, public shortcode text, play count, and relative date.
- Rows use SortableJS with the left control as drag handle. Reordering changes both preview order and submitted source URL order.
- **Sound On** / **Sound Off** controls preview muting and the submitted `mute` field.
- A row-link defect was observed: the displayed shortcode is correct, but the anchor target is the literal template `//streamable.com/{{video.shortcode}}` rather than the row's actual shortcode.

## Preset and dimension selection

When **Done** is pressed, the joiner:

1. searches preset preference order `mp4`, then `mp4-mobile`;
2. chooses the first preset present on every originally selected video;
3. throws `Error("No preset found!")` when no common preset exists;
4. walks the current reordered list and collects each chosen preset URL in that order;
5. uses the maximum top-level `height` and `width` among selected videos;
6. clears dashboard selection, navigates to `/videos`, and starts creation with `upload_source: "concat"`.

Current frontend code unconditionally prepends `https:` to every selected preset URL. Protocol-relative URLs work, but an already absolute `https://` URL becomes malformed as `https:https://...`. This was reproduced with a disposable edited source and left the resulting concat job at `status: 1`, `percent: 0` until that test output was deleted.

## Creation wire behavior

Merge creates a new video and does not replace or delete its inputs. The flow uses two authenticated JSON requests:

1. `POST /api/v1/uploads/videos` with `{"upload_source":"concat","status":1}`. The response supplies a new blank video and shortcode.
2. `POST /api/v1/transcode/<new-shortcode>` with:
    - `height` and `width` from the maxima above;
    - `mute` from the merge-page toggle;
    - `upload_source: "concat"`;
    - `urls` in the current row order;
    - the new output `shortcode`.

Transcoding is asynchronous. The creation helper records failures in local video state with an error status and message, assigns a unique eight-character local shortcode if needed, logs to Sentry, and does not automatically retry. Transcode HTTP 429 uses the upload-too-much message; other failures parse JSON `message`.

## Verification and parity notes

- Live disposable-fixture verification confirmed ordered submission of two inputs, both creation requests, and completion at `status: 2`, `percent: 100` with expected output dimensions and audio.
- Output duration was approximately the sum of the two source durations.
- The successful output, the malformed-URL test output, and all disposable source fixtures were deleted after verification. Existing account videos were unchanged.
- No corresponding merge, join, or concat implementation was found in the sibling Python project, so this is live-web behavior rather than Python parity evidence.
- No Merge Videos API is currently exposed by `streamable-rs`.
