# Shared application runtime

`cubacadabra-app` owns portable account behavior. It is a sibling of
`cubacadabra-client` (active game sessions) and `cubacadabra-engine` (simulation).
Studio uses ordinary Rust types and methods; JSON, C and WASM are outer adapters.

## Implemented slice: account usernames

All account-username entry points now use the shared model:

- iOS: Account → Username and My Cube → Basics, owned by GameViewModel.
- Android: ProfileUsernameScreen, owned by the existing GameViewModel.
- Web: My Cube → Basics, owned by one runtime for the signed-in document.

Rust owns normalization/validation, dirty/save eligibility, in-flight state,
feedback, request method/path/body, and response parsing. Hosts own text fields,
navigation/focus, HTTP transport, credentials, and projection into their existing
profile/socket/engine state. This does not move layout, platform SDKs, or an
async HTTP runtime into Rust.

The separate in-world websocket name editor still permits spaces. This slice
preserves its existing behavior and does not claim all naming rules are identical.
The server remains authoritative for authentication, moderation, uniqueness and
age requirements; client validation is not a security boundary.

## Contract and lifecycle

Create one `AppModel::default()` / native handle / `WebApp` per host lifetime,
initially signed out. Dispatch `replace_session` with account ID and username
when restoring/signing in, and null values when signing out. Every replacement
increments the session ID and invalidates queued/in-flight work, even for the
same account. Effect IDs are never reused within that model.

`begin_username_edit` resets an idle draft and feedback when opening an editor.
It does not discard a pending save. Editing during a save preserves the new
draft; an older completion must not label that new draft as saved.

```rust
use cubacadabra_app::{AppAction, AppEffect, AppModel};

let mut app = AppModel::default();
app.dispatch(AppAction::ReplaceSession {
    account_id: Some("account-1".into()),
    username: Some("Ada".into()),
});
app.dispatch(AppAction::UsernameChanged { value: "  Grace_7  ".into() });
app.dispatch(AppAction::SaveUsername {});

for effect in app.take_effects() {
    let AppEffect::HttpRequest { effect_id, account_id, method, path, body } = effect;
    // Capture credentials for account_id, execute method/path/body on the host,
    // then send the raw HTTP status/body to Rust. Never apply a raw user first.
    app.dispatch(AppAction::HttpCompleted {
        effect_id,
        status: 200,
        body: r#"{"user":{"id":"account-1","username":"Grace_7"}}"#.into(),
    });
}
assert_eq!(app.snapshot().profile.username.as_deref(), Some("Grace_7"));
```

A network failure dispatches `http_failed`. A successful username response must
contain the current account ID and submitted username. Unrecognized/malformed
responses are retryable errors. An HTTP 401 is unauthorized even without JSON.

Hosts project only the accepted snapshot's username, merging that field into
their existing user. Birthday/avatar responses merge only their own fields,
so a concurrent response cannot roll back an accepted name. Native unrelated
profile updates also check the captured session ID before applying results.
Web's combined Basics form keeps its avatar orchestration in JavaScript for now:
unchanged usernames need no HTTP request, and avatar failure preserves username
success.

Host cancellation is best-effort. Invalidating an effect prevents stale local
updates; it cannot undo a request the server has already processed. Reload/
authentication restoration reads the server again. The web document invalidates
work at logout/page exit and rechecks authentication on a back-forward-cache
restore. This is shared client behavior, not real-time synchronization of
separate logged-in devices.

## Bindings and builds

The C ABI has seven feature-independent functions: create, destroy, dispatch
JSON, snapshot JSON, poll effect JSON, output pointer and output length.
Handles are single-threaded; callers copy output before the next mutation.
Invalid JSON/actions return failure without changing state. `WebApp` has the
same dispatch/snapshot/effect contract. Snapshot protocol version 1 is checked
by Swift, Kotlin and JavaScript; binding errors do not silently select native
fallback rules.

- iOS builds the existing two static archives and copies the matching shared C
  header from the configured Rust repo into DerivedSources.
- Android's Rust build produces app/client libraries for arm64 and x86_64;
  its small byte-array JNI adapter links both and reads the shared C header.
- The existing web renderer build also emits an independent
  `public/wasm/app/cubacadabra_app.js` module. Account pages do not load the
  renderer to edit a username.

## Verification and rollout

`crates/app/tests/username-contract.json` contains shared action/expected-state/
effect scenarios. They cover normalization, invalid/unchanged drafts, duplicate
submissions, sign-out/replacement, navigation during save, newer drafts,
transport/server failures, and malformed/wrong-account/wrong-name responses.

Run:

```sh
# In rust/
cargo test --workspace
cargo clippy -p cubacadabra-app --all-targets -- -D warnings
sh scripts/build_web_renderer.sh --release

# In ios_app/ — tests the production Swift decoder and C adapter on macOS
sh scripts/check_app_contract.sh

# In web/ — tests the actual generated WASM plus host lifecycle/projection
npm run check:app
```

The iOS app build and Vite production build have also been checked. Android's
Rust crate cross-checks for both targets, but its Kotlin/JNI/APK build and device
interaction still need Android Studio verification; no Gradle/adb or Android
tests were run/added. No simulator was launched and no live account was changed.

Before widening the migration, verify on devices: save/taken/expired-session
feedback, edit while saving, leave/reopen the editor, sign out while saving,
and sign into another account. On web also check avatar-only saves and partial
username-success/avatar-failure.

This slice establishes a tested shared source of decisions, not a demonstrated
net line-count saving. The next bounded candidate is morph selection, using
these same adapters without adding feature-specific ABI functions. Catalog,
safety and age-gate migrations should wait until this boundary is proven in
device use.
