## Key Idioms & Patterns

- **AppState:**
- Single struct injected as `Extension<Arc<AppState>>`.
- Contains all singletons: `conduit`, `hub`, `auth`, `encoder`, `jwt_secret`, `config`, `auth_public_paths`, and optional `surrealdb`.

- **Feature Flags:**
- `surreal`, `datastar-sse`.
- Optional dependencies gated behind these flags; conditional compilation applies in `app_state.rs`, `builder.rs`, and `lib.rs`.

- **Module Registration:**
- Managed via the `inventory` crate (switched from `linkme` to eliminate false rust‑analyzer errors).
- The proc‑macro emits `inventory::submit!`.

- **TransportHub:**
- Holds three channel sets: global broadcast (`broadcast::Sender`), per‑topic (`HashMap<String, broadcast::Sender<String>>`), and per‑user (`HashMap<String, HashMap<Uuid, broadcast::Sender<ServerMessage>>>`).
- Uses `ConnectionHandle` with UUIDs to support multiple connections per user.

- **SSE/WS Handler Symmetry:** Both entry points utilize the same `select_message` helper, intent filtering, and token extraction (cookie, header, query param).
- **Auth:**
- Governed by the `AuthProvider` trait with `auth_routes()`, `protect_router()`, and `public_paths()`.
- The builder collects public paths into `AppState`; middleware reads them to bypass authentication.

- **Conduit:** `Arc<RwLock<Tera>>` for concurrent hot‑reload with a debounced file watcher. Frozen mode uses `rust-embed`.
- **Error Handling:** `BlennyError` enum (NotFound, Unauthorized, Internal) with an `IntoResponse` implementation producing structured JSON: `{"error":{"type":"...","message":"..."}}`.

---

## Important Decisions & Rationale

| Decision                                                      | Why                                                                                    |
| ------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `inventory` over `linkme`                                     | Eliminates false rust‑analyzer errors; same compile‑time discovery.                    |
| `Arc<Conduit>` in `AppState` (non‑optional)                   | Always present; no `expect` unwraps needed in handlers.                                |
| `transport_auth_required` defaults to `true`                  | Matches Pharo reference; secure by default.                                            |
| `public_paths()` empty default on `AuthProvider`              | The framework has no opinion; reference modules declare their own paths explicitly.    |
| `ConnectionHandle` with UUIDs                                 | Multiple tabs per user, clean unregistration.                                          |
| SSE per‑user stream merge via `tokio::select!`                | Same pattern as WebSocket; kept DRY via `select_message` helper.                       |
| SurrealDB client created in builder; fail‑fast if unreachable | Idiomatic Rust; `?` propagates error.                                                  |
| Modules split into `transport/` sub‑crate                     | Readability and isolation; public API unchanged via `mod.rs` re‑exports.               |
| `public_routes()` on `BlennyModule` removed                   | Security simplicity; all module routes protected unless declared by `AuthProvider`.    |
| Datastar encoder delegates to official SDK                    | `.into_datastar_event().write_as_axum_sse_event()`; no manual wire format maintenance. |

---

## Architectural Guardrails & Invariants (DO NOT REFACTOR AWAY)

To prevent regression during automated refactoring, agents must preserve the following architectural invariants:

- **Module Lifecycles & Self-Assembly:**
- **No Manual Main Wiring:** `main.rs` must remain minimal (under 5 lines). Never add explicit module initialization code to `main.rs`. Modules _must_ self-assemble using `#[blenny_module]` via compile-time discovery.
- **Lifecycle Hooks:** Every module must respect the `initialize_module`, `start_module`, and `stop_module` hooks, alongside its respect for the module disable flag.

- **Transport & Real-Time Defaults:**
- **Real-Time by Default:** SSE must always be active and functional. WebSocket is strictly an opt-in fallback configured via topology, never a total replacement.
- **Ubiquitous Broadcasting:** `TransportHub` must expose global, per-user, and topic pub/sub methods (`broadcast_html`, `broadcast_data`, `direct_html_to_user`, `publish`, `subscribe_topic`). Do not encapsulate or hide these capabilities away from handlers.
- **Intent-Driven Connections:** Messages must retain client or server-side filtering via connection intents (`ui`, `data`, `command`, `notification`). Standard encoders filter server-side via `?intent=`, while the Datastar encoder delegates filtering to the client. Do not unify or delete this distinction.
- **Pluggable Encoders:** The framework must remain agnostic between `StandardEncoder` (JSON envelope) and `DatastarEncoder` (official SDK) via configuration toggles.

- **Unified Assets & Rendering:**
- **Conduit Asset Routing:** All rendering must pass through `Conduit` via naming conventions (e.g., `"auth/login"` mapping to `.tera`). Modules explicitly assign template names to their respective handlers; do not hardcode absolute path strings.
- **Dual-State Asset Delivery:** Asset loading must dynamically support both Dev (hot-reload via debounced file watcher) and Prod (compiled directly into the binary via `rust-embed`).

- **Security & Infrastructure:**
- **Pluggable Auth Providers:** Authentication must strictly be driven by `AuthProvider` modules flagged with `#[blenny_auth_provider]`. The framework must autonomously discover the active provider, mount its routes, and register its public paths.
- **Multi-Layer Configuration:** System config must use `figment` to merge environment variables, `blenny.json`, and safe internal Rust defaults. Do not flatten this into a single hardcoded layer.
- **Anti-Fragile Panic Recovery:** The Tower middleware layer is an ironclad boundary. It must catch all internal execution panics and cleanly translate them into structured `BlennyError` JSON payloads.

---

## Current State (May 2026)

- All roadmap features implemented and tested.
- Integration tests pass against a real SurrealDB instance.
- Production‑ready; minor code‑quality items remain (long `serve()` method, duplicated intent logic).
- Three ports (Rust, Clojure, Smalltalk) share the same architectural principles; Rust is the primary development branch.

---

## Code Style & Idioms

- **Formatting:** Code must adhere strictly to standard `cargo fmt`.
- **Warnings:** The codebase must compile with zero warnings under `cargo clippy`.

---

## Testing

- **Unit tests:** `blenny/tests/encoder_tests.rs`, `transport_hub_tests.rs`.
- **Integration tests:** `blenny/tests/auth_integration_tests.rs`, `sse_integration_tests.rs`, `surrealdb_integration.rs`.
- **Test utilities:** `blenny/tests/test_utils/mod.rs` – `TestServer` with random port, `TestUser`, `login_and_get_cookie`, `make_authenticated_request`.
- **SurrealDB tests:** require `SURREALDB_URL` env var; skip gracefully if not set. Run via:

```bash
./test.sh surreal

```

- **Datastar tests:** behind `datastar-sse` feature. Run via:

```bash
  cargo test --features datastar-sse

```

---

## How to Navigate the Codebase

- Start at `main.rs` → `BlennyBuilder` → `serve()`.
- Module trait: `blenny/src/module.rs`.
- Real‑time: `blenny/src/transport/mod.rs` (hub, SSE, WS).
- Auth: `blenny/src/auth.rs` (trait), `blenny/src/modules/auth.rs` (example).
- Conduit: `blenny/src/conduit.rs`.
- Config: `blenny/src/config.rs`.
- Error handling: `blenny/src/error.rs`, `blenny/src/middleware.rs`.
- Proc‑macros: `blenny-macros/src/lib.rs`.

---

## Common Pitfalls

- **Proc-macro Crate Isolation:** The proc‑macro must be kept in a separate crate; `extern crate self as blenny;` is required in `lib.rs` so generated code can properly reference `blenny::ModuleRegistration`.
- **Axum Middleware Ordering:** Axum layers must be applied **after** routes for extensions to be visible to those routes.
- **SurrealDB Connection Strings:** When initializing `Surreal::new::<Ws>(url).await`, the scheme must be `Ws` (not `Client`), and the URL must be a bare `host:port` string (strip any `ws://` prefix).
- **Channel Lifetime:** `TransportHub::register_user` returns a `ConnectionHandle`. This handle **must** be kept alive for the connection lifetime, otherwise the user channel drops immediately.
- **Stream Lag Handling:** The `select_message` helper handles `Lagged` errors by logging and retrying. Do not break or exit the stream loop when a lag error occurs.

---

## Future Work

- Refactor `BlennyBuilder::serve()` into smaller private methods.
- Deduplicate intent‑parsing logic between SSE and WS handlers.
- Add a `transport_auth_required = false` test for the lockdown flag.
- Explore Rhai scripting as an optional module layer.
