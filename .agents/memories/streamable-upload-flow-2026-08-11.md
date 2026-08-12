STREAMABLE WEB UPLOAD FLOW
Captured: 2026-08-11
Source: Chrome DevTools Protocol network capture on https://streamable.com/
Fixture: media/mp4.mp4
Fixture size: 574823 bytes
Fixture media: MP4, 640x360, 13.346667 seconds, no audio
Result: successful upload and transcode; final video status=2, percent=100

## SECURITY / REDACTION

This record intentionally replaces all temporary AWS credentials, session tokens,
transcoder tokens, policy documents, signatures, account details, client IP/geo,
session IDs, cookies, authorization values, and generated shortcode values with
angle-bracket placeholders. CDN signed query strings are omitted.

## EXECUTIVE SUMMARY

The live browser flow remains structurally:

1. Obtain a shortcode plus temporary S3 upload configuration from Streamable.
2. Initialize video metadata with Streamable.
3. Upload raw file bytes directly to S3 using a freshly generated AWS SigV4 header.
4. Report upload telemetry to Streamable.
5. Ask Streamable to transcode the uploaded S3 object.
6. Fetch processed video state and thumbnail.

Important drift from the current Python-derived Rust model/signing code was observed:

- fields.acl is absent from the live upload-configuration response.
- x-amz-acl is absent from the live S3 PUT request and SignedHeaders list.
- transcoder_options contains key, not url.
- The live S3 host/path are streamables-upload.s3.amazonaws.com/upload/<shortcode>.
- Region remains us-east-1 and payload mode remains UNSIGNED-PAYLOAD.

## OBSERVED NETWORK SEQUENCE

Requests below are ordered by their observed application operations.

Core flow:

1.  GET https://api-f.streamable.com/api/v1/uploads/shortcode
2.  POST https://api-f.streamable.com/api/v1/videos/<shortcode>/initialize
3.  PUT https://streamables-upload.s3.amazonaws.com/upload/<shortcode>
4.  POST https://api-f.streamable.com/api/v1/log
5.  POST https://api-f.streamable.com/api/v1/uploads/<shortcode>/track
6.  POST https://api-f.streamable.com/api/v1/transcode/<shortcode>
7.  GET https://api-f.streamable.com/api/v1/videos/<shortcode>
8.  GET https://cdn-cf-east.streamable.com/image/<shortcode>.jpg
9.  POST https://api-f.streamable.com/api/v1/uploads/<shortcode>/track

Non-core requests observed during the same flow:

- POST https://o20911.ingest.sentry.io/api/5192543/store/ (twice)
- Browser font, favicon, local blob-media, and UI asset requests

All core requests returned HTTP 200. The initialize request emitted a
Network.responseReceived event with HTTP 200, followed by net::ERR_ABORTED while
reading its empty response. This did not interrupt the flow.

## STEP 1: OBTAIN SHORTCODE AND UPLOAD CONFIGURATION

### Request:

GET https://api-f.streamable.com/api/v1/uploads/shortcode
Body: none
Response status: 200
Response content type: application/json

### Redacted response, preserving live field names and meaningful non-secret values:

```json
{
    "accelerated": false,
    "bucket": "streamables-upload",
    "credentials": {
        "accessKeyId": "<redacted-access-key>",
        "secretAccessKey": "<redacted-secret-access-key>",
        "sessionToken": "<redacted-session-token>"
    },
    "fields": {
        "key": "upload/<shortcode>",
        "bucket": "streamables-upload",
        "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
        "X-Amz-Credential": "<redacted-access-key>/<YYYYMMDD>/us-east-1/s3/aws4_request",
        "X-Amz-Date": "<YYYYMMDDTHHMMSSZ>",
        "X-Amz-Security-Token": "<redacted-session-token>",
        "Policy": "<redacted-policy>",
        "X-Amz-Signature": "<redacted-signature>"
    },
    "url": "https://s3.amazonaws.com/streamables-upload",
    "video": "<large video metadata object; schema listed below>",
    "options": {
        "preset": "mp4",
        "shortcode": "<shortcode>",
        "screenshot": true
    },
    "shortcode": "<shortcode>",
    "key": "upload/<shortcode>",
    "time": "<epoch-seconds>",
    "transcoder": null,
    "transcoder_options": {
        "key": "upload/<shortcode>",
        "token": "<redacted>",
        "shortcode": "<shortcode>",
        "size": 574823
    }
}
```

Critical negative observation: fields has no acl member.

### Live video object field names returned here:

file_id, shortcode, status, original_name, duration, height, width,
audio_channels, user_id, ext, error, session_id, gif, reddit_title,
source_title, thumb_gif, reddit_url, hot, upload_source, trending, queued,
extractor, extract_id, privacy, subreddit, client_geo, client_ip, user_name,
ad_type, ad_tag, ad_parameters, ad_tags, paid_plays, parent_user_id,
allow_download, hide_sharing, disable_streamable, date_added, date_accessed,
percent, flagged, screenshot_bucket, poster_url, poster_file_name,
thumbnail_url, thumbnail_file_name, dynamic_thumbnail_url, title, description,
version, tags, labels, watermark_url, watermark_link, plays, url, owner_plan,
remove_branding, allowed_domain, waiting_for_best, color, original_size,
original_bitrate, original_framerate, max_version, source_url,
thumbnail_offset, files, captions, storyboards, available_file_resolutions,
privacy_settings, plan_limits.

## STEP 2: INITIALIZE VIDEO METADATA

### Request:

POST https://api-f.streamable.com/api/v1/videos/<shortcode>/initialize
Content-Type: application/json

```json
{
    "original_size": 574823,
    "original_name": "mp4.mp4",
    "upload_source": "web",
    "title": "mp4"
}
```

Observed response: HTTP 200 with no usable response body. Chrome subsequently
reported net::ERR_ABORTED for response loading, but the upload proceeded normally.

## STEP 3: SIGN AND UPLOAD RAW BYTES TO S3

### Request:

PUT https://streamables-upload.s3.amazonaws.com/upload/<shortcode>
Body: exact 574823 bytes from mp4.mp4
Response status: 200
Response content type: text/plain

### Application-controlled request headers:

Host: streamables-upload.s3.amazonaws.com
Content-Type: application/octet-stream
Content-Length: 574823
x-amz-content-sha256: UNSIGNED-PAYLOAD
x-amz-date: <YYYYMMDDTHHMMSSZ>
x-amz-security-token: <redacted>
x-amz-user-agent: aws-sdk-js/2.1530.0 callback
Authorization: AWS4-HMAC-SHA256 Credential=<redacted-access-key>/<YYYYMMDD>/us-east-1/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date;x-amz-security-token;x-amz-user-agent, Signature=<redacted>

### Browser-controlled headers also present but not signed:

Accept, Accept-Encoding, Accept-Language, Connection, Origin, Referer,
Sec-CH-UA, Sec-CH-UA-Mobile, Sec-CH-UA-Platform, Sec-Fetch-Dest,
Sec-Fetch-Mode, Sec-Fetch-Site, User-Agent.

Critical negative observation: x-amz-acl is neither transmitted nor signed.

### Canonical request shape reconstructed from observed headers:

```text
PUT
/upload/<shortcode>

host:streamables-upload.s3.amazonaws.com
x-amz-content-sha256:UNSIGNED-PAYLOAD
x-amz-date:<YYYYMMDDTHHMMSSZ>
x-amz-security-token:<redacted>
x-amz-user-agent:aws-sdk-js/2.1530.0 callback

host;x-amz-content-sha256;x-amz-date;x-amz-security-token;x-amz-user-agent
UNSIGNED-PAYLOAD
```

### Credential scope:

<YYYYMMDD>/us-east-1/s3/aws4_request

The request signs a freshly generated current UTC x-amz-date, not necessarily the
X-Amz-Date supplied in the upload-configuration response.

## STEP 4: STREAMABLE LOG AND UPLOAD TELEMETRY

### A. Browser log request

POST https://api-f.streamable.com/api/v1/log
Content-Type: application/json
Response status: 200

```json
{
    "message": "Unknown Error",
    "version": "unknown"
}
```

This log occurred after the successful S3 PUT. It did not stop the flow.

### B. Completion tracking request

POST https://api-f.streamable.com/api/v1/uploads/<shortcode>/track
Content-Type: application/json
Response status: 200

```json
{
    "event": "complete"
}
```

### C. Progress tracking request observed later

POST https://api-f.streamable.com/api/v1/uploads/<shortcode>/track
Content-Type: application/json
Response status: 200

```json
{
    "uploadPercent": 100,
    "event": "progress"
}
```

Observed ordering is completion telemetry before the transcode request, then the
100-percent progress telemetry after the final video fetch/thumbnail request.

## STEP 5: REQUEST TRANSCODING

### Request:

POST https://api-f.streamable.com/api/v1/transcode/<shortcode>
Content-Type: application/json

```json
{
    "upload_source": "web",
    "key": "upload/<shortcode>",
    "token": "<redacted>",
    "shortcode": "<shortcode>",
    "size": 574823
}
```

Response status: 200
Response content type: application/json

The response is a video metadata object. At this point its schema includes the
same main video fields from Step 1, plus size, bitrate, and updated queue/status
state. No separate wrapper object was observed.

## STEP 6: FETCH FINAL VIDEO STATE

### Request:

GET https://api-f.streamable.com/api/v1/videos/<shortcode>
Body: none
Response status: 200
Response content type: application/json

### Selected redacted response values:

```json
{
    "shortcode": "<shortcode>",
    "status": 2,
    "queued": null,
    "percent": 100,
    "original_name": "mp4.mp4",
    "duration": 13.346667,
    "width": 640,
    "height": 360,
    "audio_channels": 0,
    "original_size": 574823,
    "original_bitrate": 344540,
    "original_framerate": 29.97002997002997,
    "available_file_resolutions": ["mp4"],
    "files": {
        "mp4": {
            "status": 2,
            "percent": 100,
            "size": 539050,
            "width": 640,
            "height": 360,
            "bitrate": 323098,
            "framerate": 30,
            "duration": 13.346667,
            "audio_channels": 0,
            "reencoded": true,
            "url": "//cdn-cf-east.streamable.com/video/mp4/<shortcode>.mp4?<redacted-signed-query>",
            "poster_url": "https://cdn-cf-east.streamable.com/image/<shortcode>_first.jpg?<redacted-signed-query>",
            "thumbnail_url": "https://cdn-cf-east.streamable.com/image/<shortcode>.jpg?<redacted-signed-query>"
        }
    }
}
```

The browser then fetched:

GET https://cdn-cf-east.streamable.com/image/<shortcode>.jpg

The UI displayed "Your video is ready!", one video in the account, and a public
Streamable link for the generated shortcode.

## RUST IMPLEMENTATION STATUS

These are verified against the live response/request captured above.

1. Live UploadInfo has no acl field.

File: core/src/models/mod.rs
Live response: fields.acl is absent.

Current Rust status: Fields omits acl, matching the live response.

2. Live S3 PUT does not transmit or sign x-amz-acl.

File: core/src/utils/s3/mod.rs
Live behavior: no x-amz-acl header; SignedHeaders excludes x-amz-acl.
Current Rust status: signer omits x-amz-acl from both transmitted and signed
headers.

### Live SignedHeaders:

host;x-amz-content-sha256;x-amz-date;x-amz-security-token;x-amz-user-agent

3. TranscoderOptions uses the live field name.

File: core/src/models/mod.rs
Live response/request field: transcoder_options.key: String

Current Rust status: TranscoderOptions::key matches the live response. A future
client should send transcoder_options plus upload_source="web" to
POST /api/v1/transcode/<shortcode>.

4. S3 host, path, payload mode, user agent, and current-time signing are aligned.

### Current Rust behavior and live browser both use:

- virtual-hosted endpoint: <bucket>.s3.amazonaws.com
- path: /<fields.key>
- payload hash: UNSIGNED-PAYLOAD
- x-amz-user-agent: aws-sdk-js/2.1530.0 callback
- current UTC x-amz-date
- region parsed from X-Amz-Credential
- temporary x-amz-security-token
- Content-Type: application/octet-stream
- Content-Length equal to file byte length

## MINIMAL IMPLEMENTATION ORDER FOR THE FUTURE CLIENT FLOW

1. GET /api/v1/uploads/shortcode and deserialize UploadInfo.
2. POST /api/v1/videos/{shortcode}/initialize with original_size,
   original_name, upload_source="web", and title.
3. Build SignedS3Put without x-amz-acl.
4. PUT file bytes to SignedS3Put.url using SignedS3Put.headers.
5. POST /api/v1/uploads/{shortcode}/track with {"event":"complete"}.
6. POST /api/v1/transcode/{shortcode} with upload_source plus the live
   transcoder_options fields: key, token, shortcode, size.
7. GET /api/v1/videos/{shortcode} until status/percent indicate completion.
8. Optionally POST progress tracking events and expose returned video metadata.

## CAPTURE RESULT

Upload succeeded.
S3 PUT succeeded with HTTP 200.
Transcode request succeeded with HTTP 200.
Final video fetch succeeded with HTTP 200.
Final state: status=2, percent=100, MP4 output available.
Current UploadInfo and S3 signing assumptions match the captured successful PUT.

RAW CAPTURE: SECOND UPLOAD (2026-08-11)
Captured from Chrome tab 99935883 while uploading a second fixture.
Video URL: https://streamable.com/<shortcode>
Fixture: media/webm.webm
Fixture bytes: 382189
Fixture SHA-256: b2703c5a84123f878a7b99fd67d52155afb13c3b63dd17617c6c06066806ddfb
Capture source: Chrome DevTools Protocol Network domain.
Header maps and text bodies below are preserved exactly as exposed by CDP.
For the S3 PUT, CDP did not expose postData; REQUEST BODY identifies the exact bytes by fixture path, byte count, and SHA-256.
This section contains live raw authorization and temporary credential material.

### --- REQUEST 1 ---

REQUEST ID: 1849453.459
METHOD: GET
URL: https://api-f.streamable.com/api/v1/uploads/shortcode?size=382189&version=unknown
RESOURCE TYPE: Fetch

#### REQUEST HEADERS (requestWillBeSent):

```json
{
    "Cache-Control": "no-cache",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST HEADERS (requestWillBeSentExtraInfo):

```json
{
    "Accept": "*/*",
    "Accept-Encoding": "gzip, deflate, br, zstd",
    "Accept-Language": "en-MM,en-GB;q=0.9,en-US;q=0.8,en;q=0.7",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Cookie": "<redacted-cookie-header>",
    "Host": "api-f.streamable.com",
    "Origin": "https://streamable.com",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "same-site",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST BODY:

[no request body]
RESPONSE STATUS: 200 OK

#### RESPONSE HEADERS (responseReceived):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Type": "application/json; charset=utf-8",
    "Date": "Tue, 11 Aug 2026 21:52:03 GMT",
    "Vary": "origin,accept-encoding",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-kjyo7100043-IAD, cache-fco2270024-FCO",
    "X-Timer": "S1786485123.389515,VS0,VE211",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache",
    "content-encoding": "gzip",
    "transfer-encoding": "chunked"
}
```

#### RESPONSE HEADERS (responseReceivedExtraInfo):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Type": "application/json; charset=utf-8",
    "Date": "Tue, 11 Aug 2026 21:52:03 GMT",
    "Vary": "origin,accept-encoding",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-kjyo7100043-IAD, cache-fco2270024-FCO",
    "X-Timer": "S1786485123.389515,VS0,VE211",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache",
    "content-encoding": "gzip",
    "transfer-encoding": "chunked"
}
```

#### RESPONSE BODY:

{"accelerated":false,"bucket":"streamables-upload","credentials":{"accessKeyId":"<redacted-access-key>","secretAccessKey":"<redacted-secret-access-key>","sessionToken":"<redacted-session-token>"},"fields":{"key":"upload/<shortcode>","bucket":"streamables-upload","X-Amz-Algorithm":"AWS4-HMAC-SHA256","X-Amz-Credential":"<redacted-access-key>/20260811/us-east-1/s3/aws4_request","X-Amz-Date":"20260811T215203Z","X-Amz-Security-Token":"<redacted-session-token>","Policy":"<redacted-policy>","X-Amz-Signature":"<redacted-signature>"},"url":"https://s3.amazonaws.com/streamables-upload","video":{"file_id":null,"shortcode":"<shortcode>","status":0,"original_name":null,"duration":null,"height":null,"width":null,"audio_channels":null,"user_id":"<redacted-user-id>","ext":null,"error":null,"session_id":"<redacted-session-id>","gif":null,"reddit_title":null,"source_title":null,"thumb_gif":null,"reddit_url":null,"hot":null,"upload_source":"web","trending":null,"queued":null,"extractor":null,"extract_id":null,"privacy":0,"subreddit":null,"client_geo":"<redacted-geo>","client_ip":"<redacted-ip>","user_name":"<redacted-email>","ad_type":null,"ad_tag":null,"ad_parameters":null,"ad_tags":null,"paid_plays":50,"parent_user_id":null,"allow_download":null,"hide_sharing":null,"disable_streamable":false,"date_added":1786485124,"date_accessed":1786485124,"percent":0,"flagged":0,"screenshot_bucket":null,"poster_url":null,"poster_file_name":null,"thumbnail_url":null,"thumbnail_file_name":null,"dynamic_thumbnail_url":null,"title":"","description":"","version":0,"tags":[],"labels":[],"watermark_url":null,"watermark_link":null,"plays":0,"url":"https://streamable.com/<shortcode>","owner_plan":null,"remove_branding":false,"allowed_domain":"","waiting_for_best":false,"color":"#FFFFFF","original_size":null,"original_bitrate":null,"original_framerate":null,"max_version":0,"source_url":null,"thumbnail_offset":null,"files":{},"captions":[],"storyboards":[],"available_file_resolutions":[],"privacy_settings":{"allow_download":false,"allow_sharing":true,"allowed_domain":"","domain_restrictions":"off","hide_view_count":false,"visibility":"public","is_custom":false,"password_protected":false},"plan_limits":{"is_exceeding_free_plan_limits":false,"is_exceeding_free_plan_duration_limit":false,"is_exceeding_free_plan_size_limit":false,"should_restrict_playback":false,"has_owner_without_plan":true}},"options":{"preset":"mp4","shortcode":"<shortcode>","screenshot":true},"shortcode":"<shortcode>","key":"upload/<shortcode>","time":1786485124,"transcoder":null,"transcoder_options":{"key":"upload/<shortcode>","token":"<redacted-transcoder-token>","shortcode":"<shortcode>","size":382189}}
NETWORK RESULT: loadingFinished

### --- REQUEST 2 ---

REQUEST ID: 1849453.462
METHOD: POST
URL: https://api-f.streamable.com/api/v1/videos/<shortcode>/initialize
RESOURCE TYPE: Fetch

#### REQUEST HEADERS (requestWillBeSent):

```json
{
    "Cache-Control": "no-cache",
    "Content-Type": "application/json",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST HEADERS (requestWillBeSentExtraInfo):

```json
{
    "Accept": "*/*",
    "Accept-Encoding": "gzip, deflate, br, zstd",
    "Accept-Language": "en-MM,en-GB;q=0.9,en-US;q=0.8,en;q=0.7",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Content-Length": "89",
    "Content-Type": "application/json",
    "Cookie": "<redacted-cookie-header>",
    "Host": "api-f.streamable.com",
    "Origin": "https://streamable.com",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "same-site",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST BODY:

{"original_size":382189,"original_name":"webm.webm","upload_source":"web","title":"webm"}
RESPONSE STATUS: 200 OK

#### RESPONSE HEADERS (responseReceived):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Length": "0",
    "Date": "Tue, 11 Aug 2026 21:52:04 GMT",
    "Vary": "origin",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-khef600099-IAD, cache-fco2270024-FCO",
    "X-Timer": "S1786485124.938972,VS0,VE217",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache"
}
```

#### RESPONSE HEADERS (responseReceivedExtraInfo):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Length": "0",
    "Date": "Tue, 11 Aug 2026 21:52:04 GMT",
    "Vary": "origin",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-khef600099-IAD, cache-fco2270024-FCO",
    "X-Timer": "S1786485124.938972,VS0,VE217",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache"
}
```

#### RESPONSE BODY:

[unavailable from CDP: {"code":-32000,"message":"No data found for resource with given identifier"}]
NETWORK RESULT: loadingFailed net::ERR_ABORTED

### --- REQUEST 3 ---

REQUEST ID: 1849453.463
METHOD: PUT
URL: https://streamables-upload.s3.amazonaws.com/upload/<shortcode>
RESOURCE TYPE: XHR

#### REQUEST HEADERS (requestWillBeSent):

```json
{
    "Authorization": "AWS4-HMAC-SHA256 Credential=<redacted-access-key>/<YYYYMMDD>/us-east-1/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date;x-amz-security-token;x-amz-user-agent, Signature=<redacted-signature>",
    "Content-Type": "application/octet-stream",
    "Referer": "https://streamable.com/",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "X-Amz-Content-Sha256": "UNSIGNED-PAYLOAD",
    "X-Amz-Date": "20260811T215204Z",
    "X-Amz-User-Agent": "aws-sdk-js/2.1530.0 callback",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\"",
    "x-amz-security-token": "<redacted-session-token>"
}
```

#### REQUEST HEADERS (requestWillBeSentExtraInfo):

```json
{
    "Accept": "*/*",
    "Accept-Encoding": "gzip, deflate, br, zstd",
    "Accept-Language": "en-MM,en-GB;q=0.9,en-US;q=0.8,en;q=0.7",
    "Authorization": "AWS4-HMAC-SHA256 Credential=<redacted-access-key>/<YYYYMMDD>/us-east-1/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date;x-amz-security-token;x-amz-user-agent, Signature=<redacted-signature>",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Content-Length": "382189",
    "Content-Type": "application/octet-stream",
    "Host": "streamables-upload.s3.amazonaws.com",
    "Origin": "https://streamable.com",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "cross-site",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "X-Amz-Content-Sha256": "UNSIGNED-PAYLOAD",
    "X-Amz-Date": "20260811T215204Z",
    "X-Amz-User-Agent": "aws-sdk-js/2.1530.0 callback",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\"",
    "x-amz-security-token": "<redacted-session-token>"
}
```

#### REQUEST BODY:

[raw bytes from fixture media/webm.webm; bytes=382189; sha256=b2703c5a84123f878a7b99fd67d52155afb13c3b63dd17617c6c06066806ddfb]
RESPONSE STATUS: 200 OK

#### RESPONSE HEADERS (responseReceived):

```json
{
    "Access-Control-Allow-Methods": "GET, PUT, POST, DELETE",
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Expose-Headers": "ETag",
    "Content-Length": "0",
    "Date": "Tue, 11 Aug 2026 21:52:22 GMT",
    "ETag": "\"2995e80cc176ab57d8c4c915dd9d220a\"",
    "Server": "AmazonS3",
    "Vary": "Origin, Access-Control-Request-Headers, Access-Control-Request-Method",
    "x-amz-checksum-crc64nvme": "C2jgswbJ1YM=",
    "x-amz-checksum-type": "FULL_OBJECT",
    "x-amz-expiration": "expiry-date=\"Fri, 14 Aug 2026 00:00:00 GMT\", rule-id=\"cleanup\"",
    "x-amz-id-2": "<redacted-aws-request-id>",
    "x-amz-request-id": "<redacted-aws-request-id>",
    "x-amz-server-side-encryption": "AES256"
}
```

#### RESPONSE HEADERS (responseReceivedExtraInfo):

```json
{
    "Access-Control-Allow-Methods": "GET, PUT, POST, DELETE",
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Expose-Headers": "ETag",
    "Content-Length": "0",
    "Date": "Tue, 11 Aug 2026 21:52:22 GMT",
    "ETag": "\"2995e80cc176ab57d8c4c915dd9d220a\"",
    "Server": "AmazonS3",
    "Vary": "Origin, Access-Control-Request-Headers, Access-Control-Request-Method",
    "x-amz-checksum-crc64nvme": "C2jgswbJ1YM=",
    "x-amz-checksum-type": "FULL_OBJECT",
    "x-amz-expiration": "expiry-date=\"Fri, 14 Aug 2026 00:00:00 GMT\", rule-id=\"cleanup\"",
    "x-amz-id-2": "<redacted-aws-request-id>",
    "x-amz-request-id": "<redacted-aws-request-id>",
    "x-amz-server-side-encryption": "AES256"
}
```

#### RESPONSE BODY:

NETWORK RESULT: loadingFinished

### --- REQUEST 4 ---

REQUEST ID: 1849453.464
METHOD: POST
URL: https://api-f.streamable.com/api/v1/uploads/<shortcode>/track
RESOURCE TYPE: Fetch

#### REQUEST HEADERS (requestWillBeSent):

```json
{
    "Cache-Control": "no-cache",
    "Content-Type": "application/json",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST HEADERS (requestWillBeSentExtraInfo):

```json
{
    "Accept": "*/*",
    "Accept-Encoding": "gzip, deflate, br, zstd",
    "Accept-Language": "en-MM,en-GB;q=0.9,en-US;q=0.8,en;q=0.7",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Content-Length": "39",
    "Content-Type": "application/json",
    "Cookie": "<redacted-cookie-header>",
    "Host": "api-f.streamable.com",
    "Origin": "https://streamable.com",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "same-site",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST BODY:

{"uploadPercent":34,"event":"progress"}
RESPONSE STATUS: 200 OK

#### RESPONSE HEADERS (responseReceived):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Length": "0",
    "Date": "Tue, 11 Aug 2026 21:52:18 GMT",
    "Vary": "origin",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-kcgs7200158-IAD, cache-fco2270024-FCO",
    "X-Timer": "S1786485138.960145,VS0,VE152",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache"
}
```

#### RESPONSE HEADERS (responseReceivedExtraInfo):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Length": "0",
    "Date": "Tue, 11 Aug 2026 21:52:18 GMT",
    "Vary": "origin",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-kcgs7200158-IAD, cache-fco2270024-FCO",
    "X-Timer": "S1786485138.960145,VS0,VE152",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache"
}
```

#### RESPONSE BODY:

NETWORK RESULT: loadingFinished

### --- REQUEST 5 ---

REQUEST ID: 1849453.466
METHOD: POST
URL: https://api-f.streamable.com/api/v1/log
RESOURCE TYPE: Fetch

#### REQUEST HEADERS (requestWillBeSent):

```json
{
    "Cache-Control": "no-cache",
    "Content-Type": "application/json",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST HEADERS (requestWillBeSentExtraInfo):

```json
{
    "Accept": "*/*",
    "Accept-Encoding": "gzip, deflate, br, zstd",
    "Accept-Language": "en-MM,en-GB;q=0.9,en-US;q=0.8,en;q=0.7",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Content-Length": "47",
    "Content-Type": "application/json",
    "Cookie": "<redacted-cookie-header>",
    "Host": "api-f.streamable.com",
    "Origin": "https://streamable.com",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "same-site",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST BODY:

{"message":"Unknown Error","version":"unknown"}
RESPONSE STATUS: 204 No Content

#### RESPONSE HEADERS (responseReceived):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Date": "Tue, 11 Aug 2026 21:52:24 GMT",
    "Vary": "origin",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-khef600031-IAD, cache-fco2270024-FCO",
    "X-Timer": "S1786485145.750148,VS0,VE117",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache"
}
```

#### RESPONSE HEADERS (responseReceivedExtraInfo):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Date": "Tue, 11 Aug 2026 21:52:24 GMT",
    "Vary": "origin",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-khef600031-IAD, cache-fco2270024-FCO",
    "X-Timer": "S1786485145.750148,VS0,VE117",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache"
}
```

#### RESPONSE BODY:

[unavailable from CDP: {"code":-32000,"message":"No data found for resource with given identifier"}]
NETWORK RESULT: loadingFailed net::ERR_ABORTED

### --- REQUEST 6 ---

REQUEST ID: 1849453.467
METHOD: POST
URL: https://api-f.streamable.com/api/v1/uploads/<shortcode>/track
RESOURCE TYPE: Fetch

#### REQUEST HEADERS (requestWillBeSent):

```json
{
    "Cache-Control": "no-cache",
    "Content-Type": "application/json",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST HEADERS (requestWillBeSentExtraInfo):

```json
{
    "Accept": "*/*",
    "Accept-Encoding": "gzip, deflate, br, zstd",
    "Accept-Language": "en-MM,en-GB;q=0.9,en-US;q=0.8,en;q=0.7",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Content-Length": "20",
    "Content-Type": "application/json",
    "Cookie": "<redacted-cookie-header>",
    "Host": "api-f.streamable.com",
    "Origin": "https://streamable.com",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "same-site",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST BODY:

{"event":"complete"}
RESPONSE STATUS: 200 OK

#### RESPONSE HEADERS (responseReceived):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Length": "0",
    "Date": "Tue, 11 Aug 2026 21:52:25 GMT",
    "Vary": "origin",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-kcgs7200179-IAD, cache-fco2270024-FCO",
    "X-Timer": "S1786485145.040025,VS0,VE151",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache"
}
```

#### RESPONSE HEADERS (responseReceivedExtraInfo):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Length": "0",
    "Date": "Tue, 11 Aug 2026 21:52:25 GMT",
    "Vary": "origin",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-kcgs7200179-IAD, cache-fco2270024-FCO",
    "X-Timer": "S1786485145.040025,VS0,VE151",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache"
}
```

#### RESPONSE BODY:

NETWORK RESULT: loadingFinished

### --- REQUEST 7 ---

REQUEST ID: 1849453.468
METHOD: POST
URL: https://api-f.streamable.com/api/v1/transcode/<shortcode>
RESOURCE TYPE: Fetch

#### REQUEST HEADERS (requestWillBeSent):

```json
{
    "Content-Type": "application/json",
    "Referer": "https://streamable.com/",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST HEADERS (requestWillBeSentExtraInfo):

```json
{
    "Accept": "*/*",
    "Accept-Encoding": "gzip, deflate, br, zstd",
    "Accept-Language": "en-MM,en-GB;q=0.9,en-US;q=0.8,en;q=0.7",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Content-Length": "123",
    "Content-Type": "application/json",
    "Cookie": "<redacted-cookie-header>",
    "Host": "api-f.streamable.com",
    "Origin": "https://streamable.com",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "same-site",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST BODY:

{"upload_source":"web","key":"upload/<shortcode>","token":"<redacted-transcoder-token>","shortcode":"<shortcode>","size":382189}
RESPONSE STATUS: 200 OK

#### RESPONSE HEADERS (responseReceived):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Type": "application/json; charset=utf-8",
    "Date": "Tue, 11 Aug 2026 21:52:25 GMT",
    "Vary": "origin,accept-encoding",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-khef600070-IAD, cache-fco2270021-FCO",
    "X-Timer": "S1786485145.040956,VS0,VE225",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache",
    "content-encoding": "gzip",
    "transfer-encoding": "chunked"
}
```

#### RESPONSE HEADERS (responseReceivedExtraInfo):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Type": "application/json; charset=utf-8",
    "Date": "Tue, 11 Aug 2026 21:52:25 GMT",
    "Vary": "origin,accept-encoding",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-khef600070-IAD, cache-fco2270021-FCO",
    "X-Timer": "S1786485145.040956,VS0,VE225",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache",
    "content-encoding": "gzip",
    "transfer-encoding": "chunked"
}
```

#### RESPONSE BODY:

{"file_id":null,"shortcode":"<shortcode>","status":1,"original_name":"webm.webm","duration":null,"height":null,"width":null,"audio_channels":null,"ext":null,"error":null,"reddit_title":null,"source_title":null,"thumb_gif":null,"reddit_url":null,"hot":null,"upload_source":"web","trending":null,"queued":false,"extractor":null,"extract_id":null,"privacy":0,"subreddit":null,"ad_type":null,"ad_tag":null,"ad_parameters":null,"ad_tags":null,"allow_download":null,"hide_sharing":null,"disable_streamable":false,"date_added":1786485124,"percent":0,"flagged":0,"poster_url":null,"thumbnail_url":null,"dynamic_thumbnail_url":null,"title":"webm","description":"","version":0,"tags":[],"labels":[],"watermark_url":null,"watermark_link":null,"plays":0,"url":"https://streamable.com/<shortcode>","owner_plan":null,"remove_branding":false,"allowed_domain":"","waiting_for_best":false,"color":"#FFFFFF","original_size":382189,"original_bitrate":null,"original_framerate":null,"max_version":0,"source_url":null,"thumbnail_offset":null,"files":{},"captions":[],"storyboards":[],"available_file_resolutions":[],"privacy_settings":{"allow_download":false,"allow_sharing":true,"allowed_domain":"","domain_restrictions":"off","hide_view_count":false,"visibility":"public","is_custom":false,"password_protected":false},"size":null,"bitrate":null,"plan_limits":{"is_exceeding_free_plan_limits":false,"is_exceeding_free_plan_duration_limit":false,"is_exceeding_free_plan_size_limit":false,"should_restrict_playback":false,"has_owner_without_plan":true}}
NETWORK RESULT: loadingFinished

### --- REQUEST 8 ---

REQUEST ID: 1849453.470
METHOD: GET
URL: https://api-f.streamable.com/api/v1/videos/<shortcode>?version=0
RESOURCE TYPE: Fetch

#### REQUEST HEADERS (requestWillBeSent):

```json
{
    "Cache-Control": "no-cache",
    "Content-Type": "application/json",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST HEADERS (requestWillBeSentExtraInfo):

```json
{
    "Accept": "*/*",
    "Accept-Encoding": "gzip, deflate, br, zstd",
    "Accept-Language": "en-MM,en-GB;q=0.9,en-US;q=0.8,en;q=0.7",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Content-Type": "application/json",
    "Cookie": "<redacted-cookie-header>",
    "Host": "api-f.streamable.com",
    "Origin": "https://streamable.com",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "same-site",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST BODY:

[no request body]
RESPONSE STATUS: 200 OK

#### RESPONSE HEADERS (responseReceived):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Type": "application/json; charset=utf-8",
    "Date": "Tue, 11 Aug 2026 21:52:28 GMT",
    "Vary": "origin,accept-encoding",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-kiad7000111-IAD, cache-fco2270021-FCO",
    "X-Timer": "S1786485148.478547,VS0,VE157",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache",
    "content-encoding": "gzip",
    "transfer-encoding": "chunked"
}
```

#### RESPONSE HEADERS (responseReceivedExtraInfo):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Type": "application/json; charset=utf-8",
    "Date": "Tue, 11 Aug 2026 21:52:28 GMT",
    "Vary": "origin,accept-encoding",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-kiad7000111-IAD, cache-fco2270021-FCO",
    "X-Timer": "S1786485148.478547,VS0,VE157",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache",
    "content-encoding": "gzip",
    "transfer-encoding": "chunked"
}
```

#### RESPONSE BODY:

{"file_id":"<shortcode>","shortcode":"<shortcode>","status":2,"original_name":"webm.webm","duration":13.346667,"height":360,"width":640,"audio_channels":0,"ext":null,"error":null,"reddit_title":null,"source_title":null,"thumb_gif":null,"reddit_url":null,"hot":null,"upload_source":"web","trending":null,"queued":null,"extractor":null,"extract_id":null,"privacy":0,"subreddit":null,"ad_type":null,"ad_tag":null,"ad_parameters":null,"ad_tags":null,"allow_download":null,"hide_sharing":null,"disable_streamable":false,"date_added":1786485124,"percent":100,"flagged":0,"poster_url":"https://cdn-cf-east.streamable.com/image/<shortcode>_first.jpg?Expires=1786492348574&Key-Pair-Id=APKAIEYUVEN4EVB2OKEQ&Signature=T3QHm3IMAsjC4UQNEa0WLswAXxDNVz0xGkqGYwNQrDJiJu9Qj3v~~hm00u2kwiINkB2PKIxpOBQsVl9EKa5417Ytn9TeJJ2TRInSnq8oxsJX1L96y1BmkqxpBcUcn0IZW~~VvBPt0ELpCe00LdBLTh3F5OmSL1WeRoFe4DnS8Udbvxaxtcb3NTsSYLXOvDoFXT5xxEIMIPz-V4mJFDYOqJuFd8FW8IeVPWr8wRwy1ve4RLYzMCr0ToTIiu~~b4AosRTxPsluF-6a6wicDsYQAE3~~QsE0QNInZd4vqk7xTwpvVyadAF8B0B7PGkDGef2MLXqvqwSzvjUQKXBdEPg5bw7Dw__","thumbnail_url":"https://cdn-cf-east.streamable.com/image/<shortcode>.jpg?Expires=1786492348575&Key-Pair-Id=APKAIEYUVEN4EVB2OKEQ&Signature=WHpvXvlin04LhGXhblZlrKxo1ZokQO3NGNJH~~lY7DgYioO5Mv1jNcs~~3-6rBqu9XSVY~~Y7quw~~LCCA8W3s3S53r7S~~dT0w7oHSw8xP0EDTVKtG9GRl1hjxaD8clwUUkgpp-18XYlTMKjv9ra~~PK1Qp4BwqShmLblH~~F~~Bkc2uh5qNvggPzgrg8Ez~~9vLleYZF8cblk0Y9-CCyh1~~FPB7mEe4IBJnBKsFOh0ZV1Eqz02ADmIyjGEyuv3yDPKtmduVAn1G-Gb29kwdVtLLzZ4Jzd1qe-SF2iKhC4eiFzMgIL0f3VekdDY5Vb-LNBUNxVzTewB~~2YogtAhfYLhnKthlg__","dynamic_thumbnail_url":"//cdn-cf-east.streamable.com/image/<shortcode>.jpg","title":"webm","description":"","version":0,"tags":[],"labels":[],"watermark_url":null,"watermark_link":null,"plays":0,"url":"https://streamable.com/<shortcode>","owner_plan":null,"remove_branding":false,"allowed_domain":"","waiting_for_best":false,"color":"#FFFFFF","original_size":382189,"original_bitrate":229095,"original_framerate":29.97002997002997,"input_metadata":{"profile":"Profile 0","video_codec_name":"vp9","width":640,"height":360,"r_frame_rate":"30000/1001","avg_frame_rate":"30000/1001","refs":1,"has_b_frames":0,"level":"-99","rotation":0,"constant_frame_rate":true,"fps":"30000","duration":13.346,"bitrate":229095,"size":382189,"audio_channels":0,"has_subtitles":false,"real_framerate":29.97002997002997,"framerate":29.97002997002997,"is_stream":false},"original_height":360,"original_width":640,"original_duration":13.346,"max_version":0,"source_url":null,"thumbnail_offset":null,"files":{"mp4":{"height":360,"width":640,"size":479789,"bitrate":287578,"framerate":30,"error":null,"percent":100,"audio_channels":0,"duration":13.346667,"job_id":"mp4-<shortcode>-0","execution_time":2004,"reencoded":true,"input_metadata":{"profile":"Profile 0","video_codec_name":"vp9","width":640,"height":360,"r_frame_rate":"30000/1001","avg_frame_rate":"30000/1001","refs":1,"has_b_frames":0,"level":"-99","rotation":0,"constant_frame_rate":true,"fps":"30000","duration":13.346,"bitrate":229095,"size":382189,"audio_channels":0,"has_subtitles":false,"real_framerate":29.97002997002997,"framerate":29.97002997002997,"is_stream":false},"name":"mp4/<shortcode>.mp4","status":2,"thumbnail_file_name":"<shortcode>.jpg","poster_file_name":"<shortcode>_first.jpg","version":0,"url":"//cdn-cf-east.streamable.com/video/mp4/<shortcode>.mp4?Expires=1786744348572&Key-Pair-Id=APKAIEYUVEN4EVB2OKEQ&Signature=XOsGuRHZl~~UaqdVLCsecmDS00W0Flm6hjbhqwvJRLLXHR13cgZ3AQIhj6btcJkCITtzPqaMtilU8KcPrSqoPyt21tNCW-zzRkjokl~~bz6qlYrFD79Pz9qMkIsMr4pTIkTdZSd7C-XYhn2Ov1zKWgtK4UKlA-bQBeOE03Ia~~3AwiU7vOw0LZOwNIG8eRGR8PE7IuRwSUp4BhAAjiNaEKJnYP0oEuuWJPApK1G8vJGaSssMg73z9p1ujMPtT-WVXvLfgibUcW-HwH1qomvbO3ZBNyvvXrh3f6EI03PgumSYYoTncT3Spu8Whco9EPVC2gaA8fLwKCJrBfMWcf5EAZGw__","poster_url":"https://cdn-cf-east.streamable.com/image/<shortcode>_first.jpg?Expires=1786492348574&Key-Pair-Id=APKAIEYUVEN4EVB2OKEQ&Signature=T3QHm3IMAsjC4UQNEa0WLswAXxDNVz0xGkqGYwNQrDJiJu9Qj3v~~hm00u2kwiINkB2PKIxpOBQsVl9EKa5417Ytn9TeJJ2TRInSnq8oxsJX1L96y1BmkqxpBcUcn0IZW~~VvBPt0ELpCe00LdBLTh3F5OmSL1WeRoFe4DnS8Udbvxaxtcb3NTsSYLXOvDoFXT5xxEIMIPz-V4mJFDYOqJuFd8FW8IeVPWr8wRwy1ve4RLYzMCr0ToTIiu~~b4AosRTxPsluF-6a6wicDsYQAE3~~QsE0QNInZd4vqk7xTwpvVyadAF8B0B7PGkDGef2MLXqvqwSzvjUQKXBdEPg5bw7Dw__","thumbnail_url":"https://cdn-cf-east.streamable.com/image/<shortcode>.jpg?Expires=1786492348575&Key-Pair-Id=APKAIEYUVEN4EVB2OKEQ&Signature=WHpvXvlin04LhGXhblZlrKxo1ZokQO3NGNJH~~lY7DgYioO5Mv1jNcs~~3-6rBqu9XSVY~~Y7quw~~LCCA8W3s3S53r7S~~dT0w7oHSw8xP0EDTVKtG9GRl1hjxaD8clwUUkgpp-18XYlTMKjv9ra~~PK1Qp4BwqShmLblH~~F~~Bkc2uh5qNvggPzgrg8Ez~~9vLleYZF8cblk0Y9-CCyh1~~FPB7mEe4IBJnBKsFOh0ZV1Eqz02ADmIyjGEyuv3yDPKtmduVAn1G-Gb29kwdVtLLzZ4Jzd1qe-SF2iKhC4eiFzMgIL0f3VekdDY5Vb-LNBUNxVzTewB~~2YogtAhfYLhnKthlg__","dynamic_thumbnail_url":"//cdn-cf-east.streamable.com/image/<shortcode>.jpg"}},"captions":[],"storyboards":[],"available_file_resolutions":["mp4"],"privacy_settings":{"allow_download":false,"allow_sharing":true,"allowed_domain":"","domain_restrictions":"off","hide_view_count":false,"visibility":"public","is_custom":false,"password_protected":false},"size":479789,"bitrate":287578,"plan_limits":{"is_exceeding_free_plan_limits":false,"is_exceeding_free_plan_duration_limit":false,"is_exceeding_free_plan_size_limit":false,"should_restrict_playback":false,"has_owner_without_plan":true},"is_owner":true,"watching":0}
NETWORK RESULT: loadingFinished

### --- REQUEST 9 ---

REQUEST ID: 1849453.471
METHOD: POST
URL: https://api-f.streamable.com/api/v1/uploads/<shortcode>/track
RESOURCE TYPE: Fetch

#### REQUEST HEADERS (requestWillBeSent):

```json
{
    "Cache-Control": "no-cache",
    "Content-Type": "application/json",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST HEADERS (requestWillBeSentExtraInfo):

```json
{
    "Accept": "*/*",
    "Accept-Encoding": "gzip, deflate, br, zstd",
    "Accept-Language": "en-MM,en-GB;q=0.9,en-US;q=0.8,en;q=0.7",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Content-Length": "40",
    "Content-Type": "application/json",
    "Cookie": "<redacted-cookie-header>",
    "Host": "api-f.streamable.com",
    "Origin": "https://streamable.com",
    "Pragma": "no-cache",
    "Referer": "https://streamable.com/",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "same-site",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST BODY:

{"uploadPercent":100,"event":"progress"}
RESPONSE STATUS: 200 OK

#### RESPONSE HEADERS (responseReceived):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Length": "0",
    "Date": "Tue, 11 Aug 2026 21:52:28 GMT",
    "Vary": "origin",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-kiad7000156-IAD, cache-fco2270021-FCO",
    "X-Timer": "S1786485149.833654,VS0,VE155",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache"
}
```

#### RESPONSE HEADERS (responseReceivedExtraInfo):

```json
{
    "Accept-Ranges": "bytes",
    "Connection": "keep-alive",
    "Content-Length": "0",
    "Date": "Tue, 11 Aug 2026 21:52:28 GMT",
    "Vary": "origin",
    "Via": "1.1 varnish, 1.1 varnish",
    "X-Cache": "MISS, MISS",
    "X-Cache-Hits": "0, 0",
    "X-Served-By": "cache-iad-kiad7000156-IAD, cache-fco2270021-FCO",
    "X-Timer": "S1786485149.833654,VS0,VE155",
    "access-control-allow-credentials": "true",
    "access-control-allow-origin": "https://streamable.com",
    "access-control-expose-headers": "WWW-Authenticate,Server-Authorization",
    "cache-control": "no-cache"
}
```

#### RESPONSE BODY:

NETWORK RESULT: loadingFinished

### --- REQUEST 10 ---

REQUEST ID: 1849453.472
METHOD: GET
URL: https://cdn-cf-east.streamable.com/image/<shortcode>.jpg?Expires=1786492348575&Key-Pair-Id=APKAIEYUVEN4EVB2OKEQ&Signature=WHpvXvlin04LhGXhblZlrKxo1ZokQO3NGNJH~~lY7DgYioO5Mv1jNcs~~3-6rBqu9XSVY~~Y7quw~~LCCA8W3s3S53r7S~~dT0w7oHSw8xP0EDTVKtG9GRl1hjxaD8clwUUkgpp-18XYlTMKjv9ra~~PK1Qp4BwqShmLblH~~F~~Bkc2uh5qNvggPzgrg8Ez~~9vLleYZF8cblk0Y9-CCyh1~~FPB7mEe4IBJnBKsFOh0ZV1Eqz02ADmIyjGEyuv3yDPKtmduVAn1G-Gb29kwdVtLLzZ4Jzd1qe-SF2iKhC4eiFzMgIL0f3VekdDY5Vb-LNBUNxVzTewB~~2YogtAhfYLhnKthlg__
RESOURCE TYPE: Image

#### REQUEST HEADERS (requestWillBeSent):

```json
{
    "Referer": "https://streamable.com/",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\""
}
```

#### REQUEST HEADERS (requestWillBeSentExtraInfo):

```json
{
    ":authority": "cdn-cf-east.streamable.com",
    ":method": "GET",
    ":path": "/image/<shortcode>.jpg?Expires=1786492348575&Key-Pair-Id=APKAIEYUVEN4EVB2OKEQ&Signature=WHpvXvlin04LhGXhblZlrKxo1ZokQO3NGNJH~lY7DgYioO5Mv1jNcs~3-6rBqu9XSVY~Y7quw~LCCA8W3s3S53r7S~dT0w7oHSw8xP0EDTVKtG9GRl1hjxaD8clwUUkgpp-18XYlTMKjv9ra~PK1Qp4BwqShmLblH~F~Bkc2uh5qNvggPzgrg8Ez~~9vLleYZF8cblk0Y9-CCyh1~FPB7mEe4IBJnBKsFOh0ZV1Eqz02ADmIyjGEyuv3yDPKtmduVAn1G-Gb29kwdVtLLzZ4Jzd1qe-SF2iKhC4eiFzMgIL0f3VekdDY5Vb-LNBUNxVzTewB~2YogtAhfYLhnKthlg__",
    ":scheme": "https",
    "accept": "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
    "accept-encoding": "gzip, deflate, br, zstd",
    "accept-language": "en-MM,en-GB;q=0.9,en-US;q=0.8,en;q=0.7",
    "cache-control": "no-cache",
    "cookie": "__stripe_mid=828e7f14-b839-4da0-9c7a-6381951fbaa4880cc7; session=<redacted-session-id>; plan=; user_code=eyJhbGciOiJIUzI1NiIsImlhdCI6MTc4NjQ4MjY0MiwiZXhwIjoxNzg5MDc0NjQyfQ.eyJ1c2VyX25hbWUiOiJyc2prZ2JiZUFBNjRAcHJvdG9uLm1lIiwidXNlcl90b2tlbiI6Ik1WRFhRN1lWQVAifQ.gYDviTAgJU_XIDODT_K27ox8ErXPikgtcYVevkeiuIA; dark_mode=true; user_name=<redacted-email>; dashboard=true",
    "pragma": "no-cache",
    "priority": "u=1, i",
    "referer": "https://streamable.com/",
    "sec-ch-ua": "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Linux\"",
    "sec-fetch-dest": "image",
    "sec-fetch-mode": "no-cors",
    "sec-fetch-site": "same-site",
    "user-agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36"
}
```

#### REQUEST BODY:

[no request body]
RESPONSE STATUS: 200

#### RESPONSE HEADERS (responseReceived):

```json
{
    "accept-ranges": "bytes",
    "cache-control": "max-age=315360000",
    "content-disposition": "attachment;",
    "content-length": "21248",
    "content-type": "image/jpg",
    "date": "Tue, 11 Aug 2026 21:52:30 GMT",
    "etag": "\"cf5291b2cd15fefa57ad8c2b126202ad\"",
    "last-modified": "Tue, 11 Aug 2026 21:52:26 GMT",
    "server": "AmazonS3",
    "via": "1.1 595b5bc75f9607fd025370f043f817c2.cloudfront.net (CloudFront)",
    "x-amz-cf-id": "Fv67ER6yZRtqIYhbstaYReT5ko7R4cbijswJxDLUpuQ7fbyT7XiOMQ==",
    "x-amz-cf-pop": "MXP63-P4",
    "x-amz-server-side-encryption": "AES256",
    "x-cache": "Miss from cloudfront"
}
```

#### RESPONSE HEADERS (responseReceivedExtraInfo):

```json
{
    "accept-ranges": "bytes",
    "cache-control": "max-age=315360000",
    "content-disposition": "attachment;",
    "content-length": "21248",
    "content-type": "image/jpg",
    "date": "Tue, 11 Aug 2026 21:52:30 GMT",
    "etag": "\"cf5291b2cd15fefa57ad8c2b126202ad\"",
    "last-modified": "Tue, 11 Aug 2026 21:52:26 GMT",
    "server": "AmazonS3",
    "via": "1.1 595b5bc75f9607fd025370f043f817c2.cloudfront.net (CloudFront)",
    "x-amz-cf-id": "Fv67ER6yZRtqIYhbstaYReT5ko7R4cbijswJxDLUpuQ7fbyT7XiOMQ==",
    "x-amz-cf-pop": "MXP63-P4",
    "x-amz-server-side-encryption": "AES256",
    "x-cache": "Miss from cloudfront"
}
```

#### RESPONSE BODY:

[base64]
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
NETWORK RESULT: loadingFinished
