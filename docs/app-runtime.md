# Shared application runtime

`cubacadabra-app` is the platform-neutral domain layer for product UI outside
an active game session. SwiftUI, Compose, and the browser DOM keep ownership of
widgets, layout, navigation stacks, focus, accessibility, and platform service
integration.

```text
                    AppModel (Rust)
                   /       |       \
             actions    snapshot   effects
                /           |          \
       user/network      native UI   host HTTP,
          results                    credentials
```

The first vertical slice is account-profile username editing. Rust owns:

- trimming and validating the 2–24 character account-name contract;
- dirty, valid, saving, success, and failure state;
- consistent mappings for server error codes and presentation messages;
- effect IDs and stale-response rejection; and
- a serializable snapshot that native views can render directly.

The host dispatches edits and save requests, renders `AppModel::snapshot()`,
performs each item returned by `AppModel::take_effects()`, then dispatches the
matching success or failure action. A profile/account replacement cancels the
logical request, so any response already in flight is safely ignored.

```rust
use cubacadabra_app::{AppAction, AppEffect, AppModel};

let mut app = AppModel::new(Some("Ada".to_owned()));
app.dispatch(AppAction::UsernameChanged {
    value: "Grace_7".to_owned(),
});
app.dispatch(AppAction::SaveUsername);

for effect in app.take_effects() {
    let AppEffect::SaveUsername { effect_id, username } = effect;
    // The host POSTs username with its native HTTP/credential stack, then:
    app.dispatch(AppAction::UsernameSaved { effect_id, username });
}
```

Account profile names deliberately match the existing iOS, Android, and web
profile screens: ASCII letters, numbers, `_`, and `-`. The separate in-world
name editor currently permits spaces and is not silently changed by this
slice.

The crate exposes the same model through a native C ABI and a `WebApp`
`wasm-bindgen` wrapper. The iOS account username screen is the first end-to-end
host integration: SwiftUI renders the snapshot and the existing native
authentication service performs `SaveUsername` effects.

Next, migrate the corresponding web and Android profile screens to prove that
the serialized contract stays equally thin on all three hosts. Morph selection
should then use the same action/snapshot/effect pattern. Catalog, safety,
age-gate, and settings state can follow once those host boundaries are proven.
