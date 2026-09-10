# Shared application runtime

`cubacadabra-app` owns portable account behavior. It is a sibling of
`cubacadabra-client` (active game sessions) and `cubacadabra-engine` (simulation).
Studio uses ordinary Rust types and methods; JSON, C and WASM are outer adapters.

## Implemented slices: account profile, cube catalog, and blocked users

All account-username entry points now use the shared model:

- iOS: Account → Username and My Cube → Basics, owned by AppViewModel.
- Android: ProfileUsernameScreen, owned by AppViewModel and its AppUiState.
- Web: My Cube → Basics, owned by one runtime for the signed-in document.
- Morph/avatar saves use the same Rust action/snapshot/effect contract on all
  three hosts; the image cards remain native UI assets.
- Birthday saves use the same Rust action/snapshot/effect contract on iOS and
  web; Android accepts and projects the same snapshot fields for parity even
  though its current account UI does not edit birthdays.
- The cube catalog now uses the same Rust action/snapshot/effect
  contract on web, iOS, and Android. Rust owns page-size bounds, response
  decoding, pagination metadata, cube-id/path validation, duplicate filtering,
  loading state, and retryable feedback; each host only renders entries and
  chooses its approved package origin for the current build. Catalog loading
  is public, so its shared effect can run for a guest without attaching
  account credentials.
- Blocked-user loading, optimistic block/unblock state, rollback on failed
  requests, user-ID normalization, and moderation error mapping now use the
  same Rust action/snapshot/effect contract. Native gameplay models receive a
  read-only blocked-ID projection so the engine can continue filtering remote
  players without owning account safety state.

Rust owns normalization/validation, dirty/save eligibility, in-flight state,
feedback, request method/path/body, and response parsing. Hosts own text fields,
date pickers, navigation/focus, HTTP transport, credentials, and projection into
their existing profile/socket/engine state. This does not move layout, platform
SDKs, or an async HTTP runtime into Rust.

The separate in-world websocket name editor still permits spaces. This slice
preserves its existing behavior and does not claim all naming rules are identical.
The server remains authoritative for authentication, moderation, uniqueness and
age requirements; client validation is not a security boundary.

## Native host ownership

The iOS shell and Android Activity own sibling `AppViewModel` and
`GameViewModel` instances. Account views observe the app model directly;
`GameUiState` no longer contains authentication, profile, or app-runtime state.

- AppViewModel owns native authentication/session restoration, the profile,
  the Rust app handle and its HTTP effect tasks.
- GameViewModel owns package loading, engines/renderers, the world socket,
  gameplay state and the separate in-world name editor.
- The shell passes a read-only account/session projection to gameplay:
  session ID, account ID, native access token, accepted username and body ID.
  Credentials stay host-side, outside Rust snapshots. A newly created engine
  reapplies the latest projection.

Account restoration and profile editing do not wait for a game package or
renderer. Account UI remains available if package loading fails. Changing or
recreating a game cannot cancel an app save. Account changes invalidate pending
game loads/selections; a late guest load cannot overwrite a later selection.
Explicit sign-out clears the app session and credentials before any game work.

A world socket reporting a guest session requests app-level revalidation; it
does not clear app-owned authentication itself. Foreground refresh of an
unchanged account updates credentials without replacing the Rust session or
discarding a draft/pending save. Responses from a refresh that raced profile
work preserve the accepted local profile. Authentication jobs are canceled and
generation-fenced on sign-out/replacement so late responses cannot sign the old
account back in.

This is a bounded ownership fix, not a migration of every non-game feature.
Reporting remains a host-specific moderation flow for now; the shared slice
covers the blocked-user list and block/unblock mutations.
Android's existing auth/bridge helpers retain
their current source package. Move these only when their actual shared feature
slice warrants it; there are no placeholder repositories, use cases or
entitlement systems.

## Contract and lifecycle

Create one `AppModel::default()` / native handle / `WebApp` per host lifetime,
initially signed out. Dispatch `replace_session` with account ID, username,
and body ID on initial restoration/sign-in, and null values when signing out. Native
same-account refresh replaces only when the accepted username changed; an
unchanged refresh preserves the current session and draft. Every replacement
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
    body_id: Some("cuba:person.v1".into()),
    date_of_birth: Some("2000-01-01".into()),
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

Hosts project only accepted snapshot fields into their existing user. Birthday
responses merge only their own field, so a concurrent response cannot roll back
an accepted name or morph. Native unrelated profile updates also check the
captured session ID before applying results.
The app runtime also owns the allowed body IDs, morph draft, birthday date
validation, save eligibility, pending requests, response identity/body checks,
and user-facing save feedback. Hosts no longer issue direct username, avatar,
or birthday requests or duplicate their response/error mapping. Sign-in,
refresh, token storage, and public package/catalog downloads remain host-owned.

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

# In ios_app/ — production Swift/C bridge plus app-only host lifecycle on macOS
sh scripts/check_app_contract.sh

# In web/ — tests the actual generated WASM plus host lifecycle/projection
npm run check:app
```

The Swift lifecycle probe compiles the production AppViewModel, profile methods
and Rust bridge with fake native authentication/HTTP services, without a game
engine. It covers app-only startup, idempotent startup, refresh during edits and
saves, stale refresh completion, field-specific profile merging, logout/account
replacement, and late profile/auth responses. It does not exercise native SDKs
or real transport.

The iOS app build and Vite production build have also been checked. Android's
Rust crate cross-checks for both targets, but its Kotlin/JNI/APK build and device
interaction still need Android Studio verification; no Gradle/adb or Android
tests were run/added. No simulator was launched and no live account was changed.

Before widening the migration, verify on devices: save/taken/expired-session
feedback, edit while saving, leave/reopen the editor, sign out while saving,
and sign into another account. For the host split, also verify cold restored
sign-in with game assets unavailable, entering/changing games after a profile
edit, foreground refresh during a save, and logout/login during a package load.
On web also check birthday and avatar-only saves and partial
username-success/avatar-failure.

This establishes a tested shared source of decisions, not a promise that every
feature belongs in Rust. The catalog was a good fit because all three clients
needed the same endpoint, validation, loading, and stale-response behavior.
The blocked-user slice is the next evaluation checkpoint: if its host adapters
remain thin in device testing, reporting can be evaluated as a separate shared
effect rather than being pulled in automatically.

The web game-package deep-link resolver still performs its own paginated catalog
lookup because it needs pagination metadata to locate an arbitrary game. That
is a separate follow-up from the menu catalog state migrated here.
