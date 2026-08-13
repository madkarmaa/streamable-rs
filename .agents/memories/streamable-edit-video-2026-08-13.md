# Streamable web: Edit Video

Implementation status: **Not implemented in `streamable-rs`.**

## Entry points and availability

- A ready dashboard video exposes **Edit Video** and routes to `/edit/<shortcode>` through SPA history.
- A processing video exposes **Cancel** instead. Missing or expired shortcodes disable footer actions.
- The editor fetches `GET /api/v1/videos/<shortcode>` when cached state is absent or lacks a usable best source.
- Playback and editing use the highest-height entry in `video.files` as the best source.
- When `waiting_for_best` is true, the editor warns that the high-resolution source is still encoding and advises waiting to preserve original quality.
- The page remains in a loading state while fetching or when no usable file exists. **Save Changes** stays disabled until duration is known.

## Editing controls

- The trim control starts at `0` and the full video duration. Its two handles enforce a minimum span of `0.1` seconds.
- Start and end text fields support Arrow Up/Down adjustments of `0.03` seconds, Enter to accept the current value, Escape to restore it, and a timer button that uses the current playback position.
- Keyboard controls are Space or K for play/pause, Left or J for minus one second, Right or L for plus one second, I to set clip start, and O to set clip end.
- **Sound On** / **Sound Off** toggles mute. This control is hidden when `audio_channels === 0`.
- **Crop** initializes an inset of one sixth on every side. Crop edges and corners enforce a minimum displayed size of 50 pixels. Submission maps displayed coordinates to source pixels, clips them to video bounds, and clamps `x` and `y` to nonnegative values. **Remove Crop** clears the crop.
- **Rotate** cycles through `0`, `90`, `180`, `270`, then `0` degrees.
- **Change Thumbnail** replaces SPA history with `/thumbnail/<shortcode>`.
- Query actions `?action=crop`, `?action=mute`, and `?action=rotate` activate the corresponding initial control after video metadata loads.
- Playback failure is logged as a frontend error rather than presented as dedicated editor UI.

## Save wire behavior

**Save Changes** builds editor options with `mute` and `thumb_offset: null`. It conditionally adds:

- `start` only when clip start is truthy;
- `length` as clip end minus clip start only when clip end is truthy;
- `rotate` only when rotation is nonzero;
- `crop` as `{ width, height, x, y }` only when active and valid.

The editor then:

1. computes `version` as `(max_version || version || 0) + 1`;
2. supplies `url` from the best source with an `https:` prefix;
3. supplies the video `shortcode`;
4. optimistically marks the video `status: 1`, `percent: 0`, and updates `max_version`;
5. sends authenticated JSON to `POST /api/v1/transcode/<shortcode>`;
6. returns to the dashboard after the request resolves, while transcoding continues asynchronously.

HTTP 429 maps to `You’re uploading too much… please wait a little bit before trying again.` Other failures parse the response JSON `message`. The action logs failures to Sentry and rejects; the editor itself has no local visible error handler around Save.

## Versions and revert

- An edited video with `version > 0` exposes **Revert to Original**.
- Its confirmation modal is titled **Revert Original**, states `Are you sure you want to revert to the original version of your video?`, and provides **Close** and **Undo**.
- **Undo** sends authenticated JSON `{"targetVersion":0}` to `POST /api/v1/videos/<shortcode>/revert`, with no-cache request headers.
- A successful response replaces stored video state with the returned original version. `max_version` can remain higher than the restored `version`.
- Revert failures parse JSON `message` and are logged to Sentry, but the modal flow does not surface a dedicated error message.
- Frontend source also defines `POST /videos/<shortcode>/cancelEdits` with `{version}`, although the inspected editor UI did not expose that action.

## Verification and parity notes

- Live disposable-fixture verification confirmed mute plus 90-degree rotation: asynchronous processing reached `status: 2`, `percent: 100`, removed audio, and swapped the displayed dimensions.
- Live revert verification restored version `0`, original dimensions, and audio while retaining the higher `max_version` history value.
- The disposable fixture was deleted after verification; existing account videos were unchanged.
- No corresponding edit, retranscode, crop, rotate, or revert implementation was found in the sibling Python project, so this is live-web behavior rather than Python parity evidence.
- No Edit Video API is currently exposed by `streamable-rs`.
