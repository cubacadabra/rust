# Features Still Needed on the Path to Cubacadabra SDK v1.0.0

Status: roadmap for the SDK and platform after the 0.3 preview work.

## What v1.0.0 should mean

Cubacadabra does not need every Roblox feature before calling the SDK v1.0.0. It does need a stable, documented, supportable creator contract: a developer should be able to build a multiplayer game, publish it, reconnect to it, trust its important state, and understand failures without reaching into private implementation details.

The current `first-game`, `second-game`, and growing `third-game` demonstrate a promising code-first runtime: Luau lifecycle callbacks, worlds and geometry, interactions, UI, effects, audio, shared state, network calls, disclosure, and image-backed billboards. The preview guide also correctly calls out important limits: there is not yet a complete publishing/discovery product, a durable game database, or a cheat-resistant server-authority model.

`third-game` should continue growing as the kitchen-sink reference game and manual conformance exercise. It should not be the only test: multiplayer, malformed-input, persistence, and load behavior also need automated backend and SDK tests.

## Release phases

### Phase 0 — Preview hardening and release contract

These items should be completed before treating the SDK as a production candidate.

#### Authenticated multiplayer soak testing

Test real authenticated sessions, not only local or anonymous happy paths. The matrix should include two or more accounts, multiple browser tabs and devices, reconnects, world transitions, concurrent interactions, long-running sessions, and realistic latency, jitter, packet loss, and temporary backend failure. Increase the target player count as the backend supports it.

The test harness should record join/leave events, message IDs, server sequence numbers, reconnect duration, rejected commands, state convergence, memory growth, and errors. A soak run should end with all clients agreeing on the same authoritative snapshot and should not duplicate interactions or leak subscriptions.

`third-game` test: add a multiplayer “stress chamber” showing the authenticated player roster, connection state, server sequence, accepted/rejected intent counts, and a shared counter. Provide a reconnect action and several controls that intentionally cause concurrent updates. Use an external test runner to drive many clients and use the in-game panel for human-readable evidence.

#### Malformed and conflicting-state testing in real sessions

Exercise the complete message path with invalid JSON, missing fields, wrong types, unknown intent names, oversized payloads, stale expected versions, duplicate commands, out-of-order messages, equal-version conflicting snapshots, and simultaneous compare-and-set operations. Verify both client and backend behavior; a malformed message must be rejected safely and must never crash the game or poison a retained snapshot.

Define the conflict contract explicitly: whether the server rejects stale writes, merges specific fields, or chooses a server-authoritative result. Make state transitions deterministic, idempotent, bounded in size, and observable. Add fuzz tests and a session-level test that sends bad data through the same authenticated transport used by real games.

`third-game` test: add a development-only “chaos console” that asks the test harness to send duplicate, stale, delayed, and conflicting intents. Display the last accepted state hash, expected version, rejection reason, and recovery result. Keep the unsafe controls out of production builds.

#### SDK and package compatibility

Freeze the public API and manifest contract for v1.0.0. Specify semantic versioning, supported runtime versions, capability negotiation, unknown-field behavior, deprecation policy, migration rules, package size limits, and the difference between a breaking SDK release and a game content update.

Packages should have reproducible build output, a manifest/schema validation step, an asset inventory, a content hash, and eventually a signature or trusted provenance record. Errors should identify the package, file, field, and source location wherever possible.

`third-game` test: show the SDK/runtime version, package hash, negotiated capabilities, manifest validation result, and asset inventory in a diagnostics panel. Include fixtures for an older valid manifest, an unknown optional field, and an intentionally invalid package so forward compatibility and failure messages are visible.

#### Asset, audio, and browser-runtime robustness

The current image billboard and audio work prove the path, but v1 needs predictable behavior for decode failures, unsupported formats, missing assets, autoplay restrictions, rapid repeated triggers, concurrent sounds, asset caching, and package size limits. Define whether audio is one-shot, looped, positional, or music; add cooldown/polyphony rules so a state retry cannot create repeated or distorted playback.

`third-game` test: create an asset gallery with the billboard, every supported sound mode, rapid-trigger controls, and a missing/invalid development fixture. Show load status and a readable fallback when an asset cannot be decoded.

#### UI, input, and accessibility conformance

Verify the runtime at 390x844, 768x1024, 1280x800, and 1440x900, including safe areas, keyboard and touch input, focus order, readable contrast, text overflow, reduced motion, and no unintended horizontal scrolling. Document the supported input model and provide accessible names for interactive controls.

`third-game` test: add a responsive UI lab that displays the active viewport, focus target, input source, reduced-motion setting, and layout warnings. Exercise the same controls with keyboard, mouse, and touch-sized targets.

#### Documentation and diagnostics

The developer guide needs a stable API reference, complete request/response examples, lifecycle ordering, error taxonomy, limits, security assumptions, local development instructions, and troubleshooting for common failures. The CLI/runtime should expose useful logs without leaking credentials or internal secrets.

`third-game` test: make the game self-describing. Every major probe should show the API it exercises, the input, the result, and a short failure explanation. Link each probe to the corresponding guide section.

### Phase 1 — Durable authority and persistence

These are the main differences between a compelling multiplayer preview and a platform creators can safely ship on.

#### Durable game authority

Decide and document the authority model. Cooperative client-authored retained state is suitable for casual demonstrations, but competitive scores, inventories, progression, matchmaking, and permissions require server-authoritative commands or validators. The server must be able to reject forged results instead of trusting a client-provided score.

The authority API needs authenticated identity, idempotency keys, validation, atomic transactions, versioned snapshots, replay/audit records where appropriate, rate limits, quotas, and a clear behavior during reconnects and server restarts. Do not allow arbitrary untrusted server code to access platform secrets; provide a constrained server-side execution model if custom game logic is needed.

`third-game` test: build an “authority lab” with a server-validated score, nonce-backed objective completion, and a deliberately forged client result. Restart or reconnect the session, prove that accepted progress remains, and show the server event timeline and rejected forgery.

#### Durable player and game data

Separate ephemeral session state, durable game state, and durable player state. Define schemas, migrations, ownership, quotas, transactional updates, retention, deletion/export behavior, and what happens when a player joins from a second device. Cover profiles, settings, achievements, inventory, unlocks, and game-wide progress only where the product actually promises them.

`third-game` test: add a profile vault with a small inventory, achievement, and settings record. Change them, leave and rejoin with the same account, migrate a deliberately older test schema, and verify that a second account cannot read or modify the first account’s data.

#### Reliable real-time sessions

Specify message IDs, acknowledgements, ordering, retry and deduplication behavior, resume cursors, presence, room lifecycle, graceful shutdown, and horizontal scaling. Test reconnects during an interaction, a world load, and a persistence write. Make server sequence and client version semantics consistent across the Rust/native and web hosts.

`third-game` test: have two players race to operate the same probe while one client disconnects at each step. On return, show the resumed cursor, duplicate suppression, final state, and whether the session recovered or was intentionally restarted.

#### Permissions, teams, and private sessions

Add a stable identity and permission model for owners, collaborators, players, spectators, teams, private rooms, invite-only sessions, and moderation actions. Permissions must be enforced by the backend, not merely hidden in UI.

`third-game` test: create owner, player, spectator, and team roles. Let each role attempt the same set of actions and show the server decision. Add a private-room invite flow in the test environment.

### Phase 2 — Publishing, discovery, and creator workflow

#### Publishing and release management

Make publishing a first-class workflow: draft, validate, preview/stage, publish, unpublish, rollback, and immutable version selection. Validate scripts, manifests, assets, permissions, package size, and platform compatibility before release. Preserve release notes and make it possible to identify exactly which build a player is running.

`third-game` test: add a publishing-readiness panel that simulates draft → validation → staged → published. Show failures for a missing asset, invalid schema, oversized package, and unsupported capability; then show the package hash, release notes, and rollback to the previous version.

#### Discovery and game identity

Decide the minimum discovery product for v1: creator identity, title, description, thumbnail, tags, genre, age rating, supported platforms, visibility (private, unlisted, public), version, and basic play analytics. Search, sorting, recommendations, favorites, sharing links, and “continue playing” can be staged, but creators need a dependable way for players to find a published game.

`third-game` test: give the game a real listing preview using its billboard image as the thumbnail source. Exercise private/unlisted/public visibility, a share link, version display, and a basic launch count. Keep ranking logic out of the SDK unless it is intentionally part of the platform contract.

#### Creator tooling

Provide a reliable project initializer, local preview, watch/reload loop, package validator, asset diagnostics, logs, source locations in errors, test fixtures, and CI-friendly build commands. The guide should include a minimal first game, a multiplayer example, a persistence example, and a complete reference game assembled from supported APIs.

`third-game` test: add a “copy this probe” view that shows the smallest Luau snippet for each capability and reports whether the current package is using the documented contract. Run the same package through local preview and the staged host.

#### Content and media breadth

Decide which content types are v1: multiple images and billboards, textures, sprites, fonts, localization files, 3D models, materials, animation, particles, music, and sound groups. For each type specify format, dimensions, size, caching, licensing metadata, fallback behavior, and whether it is static package content or dynamically loaded.

`third-game` test: grow the asset gallery into a small showcase with multiple image billboards, animated effects, ambient music, positional sound, localized signs, and a deliberately missing asset. Every item should report its load and fallback state.

#### Code-first versus visual authoring

Make an explicit scope decision. A visual editor is valuable for Roblox-style accessibility, but it is not required to make a code-first SDK v1 credible if the CLI, schemas, examples, preview loop, and diagnostics are excellent. If an editor is deferred, state that clearly and avoid designing APIs that depend on editor-only metadata.

`third-game` test: keep the scene reproducible entirely from the manifest and Luau source. A future editor can import/export that package without becoming a hidden requirement for the reference game.

### Phase 3 — Platform trust, safety, and operations

#### Security and sandboxing

Threat-model package scripts, network calls, assets, authentication tokens, cross-origin access, resource exhaustion, and malicious clients. Enforce CPU, memory, message, storage, asset, and session limits. Keep secrets server-side, validate every backend boundary, and make package permissions visible to creators and players.

`third-game` test: add a development-only adversarial probe for oversized payloads, excessive interaction frequency, invalid asset references, and unauthorized calls. Verify bounded failure, rate-limit feedback, and recovery without exposing credentials.

#### Moderation, safety, and privacy

Before public UGC, provide reporting, blocking, age/content labels, creator ownership, takedown, appeals or review workflow, and privacy controls. Define how chat, images, audio, scripts, usernames, and external links are handled. A platform competing with Roblox needs safety as a core product property, not a later add-on.

`third-game` test: use test accounts to report a game or player, block a player, verify the blocked state across reconnect, and display the game’s age/content labels. Keep the test content synthetic and clearly marked.

#### Observability and supportability

Ship health checks, structured logs, metrics, traces or correlation IDs, crash/error reporting, backend dashboards, release health, and an incident/runbook process. Track join failures, reconnects, state conflicts, publish failures, asset failures, latency, and resource usage.

`third-game` test: expose a creator-safe diagnostics view with a session ID, package version, server region, round-trip latency, reconnect count, and last error code. Ensure secrets and private player data are redacted.

#### Performance and scale

Set budgets for startup time, frame time, memory, package download, asset decode, network bandwidth, concurrent players, and retained-state size. Test low-end hardware and long sessions, not only a powerful development machine.

`third-game` test: add a benchmark course that loads the largest supported scene, image, audio set, UI state, and player count for the target tier. Record cold start, warm start, frame timing, memory, and network totals.

#### Platform and host parity

The web host and Rust/native renderer should agree on manifest validation, lifecycle timing, input semantics, audio behavior, image handling, networking, and error behavior. Decide the v1 platform matrix and test supported browsers, desktop, mobile, and any native host before promising parity.

`third-game` test: run the same package and scripted interaction trace on every supported host, then compare state transitions, emitted events, screenshots where useful, and error results.

#### Economy and monetization (only if in v1 scope)

If Cubacadabra will support purchases, entitlements, creator revenue, trading, or virtual currency in v1, define a server-authoritative ledger, receipts, refunds, fraud controls, regional/platform rules, parental controls, and auditability. Otherwise explicitly defer the economy and avoid placeholder APIs that imply it is safe to use.

`third-game` test: use a fake-money test environment with a server-issued entitlement, duplicate receipt, refund, and unauthorized purchase cases. Never connect the kitchen-sink game to real payments.

## v1.0.0 release gate

Call the SDK v1.0.0 when the following are true, with evidence in CI or a release report:

1. The public API and manifest schemas are versioned, documented, validated, and covered by compatibility fixtures.
2. Authenticated multiplayer soak runs complete at the target player/session size with convergence, reconnect, memory, and error budgets defined and met.
3. Malformed, duplicate, stale, conflicting, and adversarial state messages are rejected or resolved deterministically in real authenticated sessions.
4. Important game state has a documented durable-authority and persistence model, including permissions, migrations, quotas, and recovery after restart.
5. A minimum publishing workflow exists for validation, staging, immutable versions, rollback, visibility, and release identification.
6. Creators have a usable CLI/preview/debug workflow and a guide that covers the supported APIs with runnable examples.
7. Security, moderation/privacy, observability, performance, and supported-host checks have owners and tested minimum behavior.
8. `third-game` exercises the complete supported surface and is run on every release, while dedicated automated tests cover scale and failure modes that are impractical to prove visually.

## What can wait beyond v1

These are strategically important but do not need to block the first stable SDK unless the product promise depends on them: a full visual editor, an enormous asset marketplace, sophisticated recommendation ranking, creator economy, advanced NPC/AI services, large-scale user-generated social systems, complex physics authoring, and broad console distribution.

The immediate priority is trust. `third-game` should keep adding visible probes for authority, persistence, publishing, discovery, security, and scale while the underlying behavior is proven by repeatable automated tests. On the current evidence, it is a strong preview/MVP vehicle, but it is not yet enough evidence for a production v1.0.0 SDK release.
