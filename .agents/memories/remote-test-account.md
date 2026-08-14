# Shared remote-test account

Feature-gated remote tests read `EMAIL` and `PASSWORD` from the repository-root
`.env`. The tracked `.env.example` contains empty placeholders; `.env` is ignored
and is the only file that stores real credentials.

All remote tests except registration tests use one lazily authenticated client
and one account for the test process. `REMOTE_TEST_LOCK` still serializes remote
work. This avoids repeated login requests and authentication rate limits. Remote
resource tests keep their created videos and labels instead of deleting them.
Registration tests continue creating new users because account creation is the
behavior under test. Password tests restore the shared password before ending.
