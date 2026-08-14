# Shared remote-test account

Feature-gated remote tests read `EMAIL` and `PASSWORD` from the repository-root
`.env`. The tracked `.env.example` contains empty placeholders; `.env` is ignored
and is the only file that stores real credentials.

All remote tests except registration tests use one lazily authenticated client
and one account for the test process. `REMOTE_TEST_LOCK` still serializes remote
work. This avoids repeated login requests and authentication rate limits. Remote
resource tests keep their created videos and labels instead of deleting them.
They also keep assigned labels and changed video or account privacy settings;
remote tests do not clear or reset those changes. Password tests are the only
exception and restore the shared password so later tests can sign in.
Registration tests continue creating new users because account creation is the
behavior under test.
