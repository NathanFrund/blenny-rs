# Blenny‑rs – Design & Architecture Blueprint

## 🔮 Project Identity

- **Goal:** Port the philosophy of Blenny (Pharo Smalltalk) to idiomatic Rust – a web framework where modules self‑assemble, real‑time is default, and deployment is a single binary.
- **Repository:** `blenny-rs` (GitHub)
- **Crate name:** `blenny` (published as `blenny` on crates.io)
- **Tagline:** “No bespoke server setup. Just add a struct.”

## 🎯 Core Design Principles (brought from Smalltalk)

1. **Self‑Assembling Modules** – Create a struct, mark it `#[blenny_module]`, and it becomes part of the server. No manual registration, no `main.rs` edits.
2. **Real‑Time by Default** – SSE (and optional WebSocket) are first‑class. Broadcasting to all clients requires no extra plumbing.
3. **Unified Rendering (Conduit)** – Templates are requested by name (e.g., `"auth/login"`); the framework automatically handles HTMX full‑page vs. fragment responses and extension stripping.
4. **Frozen or Live Assets** – Develop with hot‑reloading from disk; ship a single binary with all templates embedded.
5. **Broadcasting from Anywhere** – A `TransportHub` allows any code to push server‑sent events; direct per‑user messaging is built in.
6. **Pluggable Auth** – A module can _become_ the auth UI and logic; swapping it requires no rewiring.
7. **Minimal Ceremony** – The entry point (`main.rs`) is 5 lines. No Makefile codegen, no registry files.
8. **Inter‑Module Communication via Message Bus** – A shared `TransportHub` acts as the central nervous system. Modules can publish messages to named topics and subscribe to them, enabling decoupled communication.
9. **Connection Intents (Message Filters)** – Every real‑time message belongs to one of four categories: `ui`, `data`, `command`, or `notification`. With the **standard SSE encoder**, clients can optionally subscribe via a `?intent=ui,notification` query parameter; if omitted, all message categories are sent, keeping the client as simple as possible. When the **Datastar encoder** is active, the Datastar SDK provides native event listeners for each category, making server‑side filtering unnecessary – the client does the filtering. In both cases, module code tags a message with a category and the framework ensures it reaches the right clients.
10. **Pluggable Transport Encoders** – The SSE/WS transport layer can be configured to use different wire formats (e.g., Blenny’s standard JSON envelope, Datastar) without changing module code.
11. **Multi‑Layer Configuration** – Settings are merged from command‑line arguments, environment variables, a JSON file, and embedded defaults, in that priority order. Only overrides need to be specified. Every major feature (port, JWT secret, encoder, WebSocket availability, etc.) is controllable via configuration.
12. **Anti‑Fragile Middleware** – Every handler response is wrapped by default to prevent server‑side crashes and enforce a consistent shape.
13. **Module Lifecycle & Control** – Modules have `initialize_module` (after injection, before routes), `start_module`, `stop_module`, and can be disabled via a simple flag without removing code.
14. **Template Ownership** – Modules tell their handlers which templates to use, keeping core handlers template‑agnostic.

## 🦀 Technical Architecture

### Workspace Layout

```
blenny-rs/
├── Cargo.toml          # Workspace definition
├── blenny/             # Main crate (lib + binary)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs     # Entry point
│   │   ├── lib.rs      # Framework core (traits, Conduit, builder)
│   │   ├── modules/
│   │   │   ├── mod.rs
│   │   │   ├── auth.rs # Example module (auto‑discovered)
│   │   │   └── dashboard.rs
│   ├── templates/      # Tera templates (hot‑reload in dev)
│   ├── static/         # Static assets (hot‑reload in dev, embedded in prod)
├── blenny-macros/      # Proc‑macro crate
│   ├── Cargo.toml
│   └── src/lib.rs
└── app/                # (optional separate binary)
```

### Module System (Self‑Assembly)

- **Trait:** `BlennyModule` (in `blenny::`) – provides `name()`, `register_routes(router) -> Router`, and optional lifecycle hooks (`initialize_module`, `start_module`, `stop_module`).
- **Disable flag:** If a module returns `false` from a default `is_enabled()` method, it is skipped during discovery. This allows modules to be turned off without deleting them.
- **Proc Macro:** `#[blenny_module]` generates an `inventory::submit!` that pushes a `ModuleRegistration` (name + constructor) into a global collector.
- **Registry:** `inventory::collect!(ModuleRegistration)` gathers all submissions at compile time, replacing the earlier `linkme` approach (which caused false rust‑analyzer errors).
- **Builder:** `BlennyBuilder::serve()` iterates over `inventory::iter::<ModuleRegistration>`, constructs each module, calls `initialize_module`, injects dependencies, registers routes, then calls `start_module`.
- **`extern crate self as blenny;`** added to `lib.rs` so proc‑macro generated code (`blenny::ModuleRegistration`) works inside the crate. (Don’t remove this line; it’s necessary because the macro crate writes the path `blenny::ModuleRegistration`.)

### Conduit (Template Engine)

- Wraps a `tera::Tera` instance.
- **Modes:** `hot_reload(path)` (dev) reads from disk with file‑watching and debounced automatic reload; `frozen()` (prod) embeds templates via `rust-embed` into the binary.
- **Extension stripping:** `render()` automatically appends `.tera` if the caller omits it, allowing modules to request `"auth/login"` while files are `auth/login.tera`.
- **Template ownership:** Modules assign template names to their handlers (e.g., `login_handler.set_template("auth/login")`), keeping the handler logic reusable.
- **Injection:** Conduit is stored as `Arc<Conduit>` and injected into the Axum router via the unified `AppState`. Handlers extract `Arc<AppState>` and access `state.conduit`.

### TransportHub (Message Bus & Real‑Time)

- The `TransportHub` holds a Tokio broadcast channel for real‑time server‑to‑client events (SSE/WS), and it also serves as an **internal message bus**.
- **Connection Intents:** Each message is tagged with a category (`ui`, `data`, `command`, `notification`).
  - **Standard SSE encoder:** Clients connect to `/sse`. If no `?intent=` parameter is given, every message is sent (the client receives all categories). When a client specifies `?intent=ui,notification`, the transport layer filters server‑side and only forwards messages whose category matches the subscription.
  - **Datastar encoder:** The Datastar SDK maps categories to named SSE event types (`datastar‑patch‑elements`, `datastar‑patch‑signals`, etc.). Clients use native event listeners to receive only the categories they care about, so the `?intent=` query parameter is **ignored** on the Datastar endpoint. Server‑side filtering is not needed.
    In both cases, module code simply tags a message with a category; the rest is handled by the framework.
- **Pluggable Encoders:** The SSE/WS bridge can be configured to use a `BlennyStandardEncoder` or a `DatastarEncoder`, changing the wire format without affecting modules.
- **Direct Per‑User Messaging:** The hub provides `direct_message(user_id, payload)` – messages are only sent to the specific user’s connections (after authentication).
- **Topic‑Based Pub/Sub:** Implemented with a `HashMap<String, broadcast::Sender<String>>`. Modules can publish to named topics and subscribe to them, decoupling inter‑module communication.
- **WebSocket Sidecar (opt‑in):** When `websocket: true` is set in the configuration, the builder mounts a `/ws` endpoint. The WebSocket handler shares the same `TransportHub` – authenticated users automatically receive their personal channel, and intent filtering is applied identically to SSE. The feature is **disabled by default**; enabling it has no effect on the existing HTTP/SSE surface.
- **Backpressure:** Tokio’s broadcast channel drops messages for slow consumers. The buffer size is currently fixed (256); future enhancement will make it configurable and log warnings on drops.

### Transport Security Model

Blenny‑rs real‑time transports (`/sse`, `/ws`) require
authentication **by default**. A connection without a valid JWT
(cookie or `Authorization` header) is immediately rejected with
a `401 Unauthorized`.

This matches the **Pharo Blenny reference implementation** and is
grounded in a simple design principle:

> Identity is a prerequisite for real‑time messaging. Knowing
> _who_ is connected is more important than knowing _what_ they
> can access.

- The transport layer establishes identity.
- Module code (via `broadcast_html` vs `direct_html_to_user`)
  controls visibility.
- Public broadcasts are still possible – an authenticated user
  with a known identity can subscribe to a public dashboard
  stream.
- The old “public by default” model is rejected in favor of
  **secure by default**, consistent across both the Pharo and
  Rust implementations.

For the rare case where a fully public transport is desired
(e.g., a live sports scoreboard), the configuration flag
`transport_auth_required` can be set to `false`.

### Configuration System

- Configuration is built from four layers, merged in priority order:
  1. Command‑line arguments (`--key=value`)
  2. Environment variables (`BLENNY_*`)
  3. `blenny.json` file (or application‑specific JSON)
  4. Embedded defaults
- Only overrides need to be specified; every key has a sensible default.
- **Current configurable fields** (via `BlennyConfig` struct):
  - `port` (u16) – server port (default 8081)
  - `template_dir` (Option<String>) – path to templates for hot‑reload; `None` = use embedded (production)
  - `jwt_secret` (String) – signing secret for JWT tokens
  - `encoder` (String) – `"standard"` or `"datastar"`; selects the SSE transport encoder
  - `websocket` (bool) – if `true`, the `/ws` endpoint is active (default `false`). When `false`, only SSE is available.
  - `database_url` (Option<String>) – when the `surreal` feature is active, the URL of the SurrealDB instance to connect to. If absent, no client is created.
- The `figment` crate is used to merge the sources, matching the original Blenny’s composite configuration provider.

### Middleware (Anti‑Fragile)

- All module route handlers are automatically wrapped with an `AntiFragileLayer` that catches panics and returns a structured JSON error response (500 Internal Server Error) instead of crashing the server.
- Infrastructure routes (`/sse`, `/ws`, `/static`, `/health`) bypass this layer and remain as plain Axum handlers.
- A `BlennyError` enum (using `thiserror`) provides consistent error variants (`NotFound`, `Unauthorized`, `Internal`) that map to HTTP status codes and JSON bodies.

### Auth System (Pluggable)

- **Trait:** `AuthProvider` – provides `auth_routes()` (login/logout endpoints) and `protect_router()` (applies JWT validation middleware).
- **Auto‑Discovery:** A second proc‑macro `#[blenny_auth_provider]` registers an `AuthRegistration` in the inventory. The builder picks the first one and uses it.
- **Middleware Logic:** The protect layer is applied **before** auth routes are merged, so login is public; module routes are behind the guard.
- **Browser Login Flow:** GET `/login` serves a Tera form. POST `/login` accepts form data, sets a JWT cookie (`blenny_token`), and redirects to `/dashboard`. `/logout` clears the cookie.
- **User Injection:** Handlers extract `Extension<User>` to know who is logged in.
- **JWT refresh / sliding windows** are not yet implemented; the current system uses time‑based expiry.

### Infrastructure vs. Module Routes

- Routes added by modules (`BlennyModule::register_routes`) go through the full middleware pipeline (anti‑fragile + auth).
- Framework‑added endpoints (`/health`, `/sse`, `/ws`, `/static`) are mounted outside that pipeline, so they remain accessible even when auth is enabled. Authentication for these (e.g., requiring a token on `/sse`) is implemented via query parameters or custom logic within the endpoint itself.

### Module Lifecycle

1. **Discover** – `BlennyBuilder` iterates over the inventory; skips disabled modules.
2. **Instantiate** – Constructor called.
3. **Initialize** – `initialize_module(state: Arc<AppState>)` called on the module. Dependencies injected, templates assigned.
4. **Register Routes** – Each module’s `register_routes(router)` is called.
5. **Start** – `start_module()` called on every module.
6. **Stop** – On graceful shutdown, `stop_module()` called in reverse order.

### Service Bundle (AppState)

- All framework singletons are available through a single `AppState` struct injected as `Extension<Arc<AppState>>`:
  ```rust
  pub struct AppState {
      pub conduit: Option<Arc<Conduit>>,
      pub hub: Arc<TransportHub>,
      pub auth: Option<Arc<dyn AuthProvider>>,
      pub encoder: Arc<dyn TransportEncoder>,
      pub jwt_secret: String,
      pub public_paths: HashSet<String>,
  }
  ```

```

- Handlers extract `Extension<Arc<AppState>>` and access only the fields they need.

### Error Handling Strategy

- A `BlennyError` enum (via `thiserror`) provides a unified error type for modules.
- The anti‑fragile middleware catches panics and converts them into `BlennyError::Internal` JSON responses.
- For explicit errors in handlers, returning `Result<T, BlennyError>` is supported and results in the appropriate JSON error body.

### Static Assets

- CSS, JS, images and other static files are served from the `/static/*` path.
- **Development:** `tower_http::services::ServeDir` is used for instant hot‑reload.
- **Production:** `rust-embed` embeds the entire `static/` directory into the binary.
- The route is automatically mounted by `BlennyBuilder`. No manual setup is needed.

### Dependency Stack (key crates)

- **axum** + **tokio** – async web server
- **tower‑http** – middleware (CORS, tracing, static files)
- **tera** – template engine
- **inventory** – compile‑time module registration
- **rust‑embed** – embed templates / static files
- **notify** – hot‑reload file watcher
- **jsonwebtoken** – JWT authentication
- **serde** / **serde_json** – serialization
- **chrono** – timestamps for JWT
- **figment** – multi‑layer configuration
- **thiserror** – error handling
- **surrealdb** (optional) – SurrealDB client behind the `surreal` feature flag

## 🧭 Roadmap & Implementation Status

| Feature                                                  | Status         |
| -------------------------------------------------------- | -------------- |
| Self‑assembling modules (`#[blenny_module]`)             | ✅ Implemented |
| Conduit (Tera) rendering with hot‑reload                 | ✅ Implemented |
| Frozen production mode (embed templates)                 | ✅ Implemented |
| Real‑time SSE broadcast (`TransportHub`)                 | ✅ Implemented |
| Module life‑cycle hooks (`start/stop`)                   | ✅ Implemented |
| Pluggable authentication (JWT)                           | ✅ Implemented |
| Service bundle (`AppState`)                              | ✅ Implemented |
| Topic‑based pub/sub for inter‑module messaging           | ✅ Implemented |
| Connection intents (message filtering)                   | ✅ Implemented |
| Pluggable transport encoders                             | ✅ Implemented |
| Multi‑layer configuration                                | ✅ Implemented |
| Anti‑fragile middleware                                  | ✅ Implemented |
| Direct per‑user messaging                                | ✅ Implemented |
| Per‑route auth control (`public_routes()`)               | ❌ Rejected |
| Static asset management (CSS, JS)                        | ✅ Implemented |
| WebSocket sidecar (opt‑in via config)                    | ✅ Implemented |
| Unified error handling (`BlennyError`)                   | ✅ Implemented |
| Datastar SSE encoder (via SDK)                           | ✅ Implemented |
| SurrealDB integration                                    | ✅ Implemented |
| Dev‑friendly proc‑macro improvements (path prefix, etc.) | ⬜ Planned     |

## 📝 Key Architectural Decisions

| Decision                                             | Rationale                                                                                                                                                     |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`inventory` over `linkme`**                        | Eliminates false rust‑analyzer errors, better DX. Same compile‑time discovery.                                                                                |
| **`Arc<Conduit>` as part of `AppState`**             | Shared, cheap to clone, single injection point.                                                                                                               |
| **Layers applied after routes**                      | Axum requires that extensions/middleware wrap all routes; adding after route registration ensures availability.                                               |
| **Conduit strips extensions**                        | Keeps module template names clean (`"auth/login"`) while files stay `.tera`.                                                                                  |
| **Proc‑macro in separate crate**                     | Required by Rust for proc‑macros. Re‑exported with `pub use blenny_macros::blenny_module`.                                                                    |
| **`extern crate self as blenny`**                    | Allows generated code to refer to `blenny::ModuleRegistration` from inside the crate. Do not remove.                                                          |
| **Auth protect layer before auth routes**            | Keeps login routes public while protecting module routes.                                                                                                     |
| **Infrastructure routes bypass global middleware**   | Avoids double authentication and keeps health/SSE/static endpoints simple.                                                                                    |
| **Connection intents via query params**              | Mirrors the Smalltalk model; lightweight and easy to implement with Tokio channels.                                                                           |
| **Pluggable encoders as configuration**              | Allows swapping wire format (Standard vs Datastar) without code changes.                                                                                      |
| **Feature flags for dev/prod**                       | `#[cfg(debug_assertions)]` currently used for static assets; will be replaced by a `hot-reload` feature for finer control.                                    |
| **Topic‑based pub/sub as a first‑class hub feature** | Unlocks decoupled inter‑module communication within the same transport infrastructure.                                                                        |
| **Intent system encoder agnosticism**                | The four categories are stable; filtering responsibility shifts from server (standard encoder) to client (Datastar). No module code changes between encoders. |
| **WebSocket as configuration opt‑in**                | WebSockets are not always needed; making them a config flag keeps the framework lightweight and matches the original Blenny’s optional transports design.     |

## 🧘‍♀️ Philosophy Summary

Blenny‑rs is not a direct copy of the Smalltalk implementation; it’s a re‑expression of Blenny’s principles using Rust’s strengths: compile‑time safety, zero‑cost abstractions, and a rich type system. The result is a framework where you drop a struct and it becomes a living part of the server – exactly the magic that made the original Blenny special. Every design choice, from connection intents to pluggable transport encoders, is aimed at preserving the friction‑free, “just works” developer experience while embracing Rust’s idioms.

## 📋 Post‑Review Refinements

- **Bundled State:** All singletons are now grouped into an `AppState` struct, simplifying injection and future `State` migration.
- **Module Lifecycle Robustness:** Shutdown timeouts, panic handling, and clear ordering guarantees added to the lifecycle specification.
- **Backpressure Awareness:** Documented the broadcast buffer behavior and future configurability.
- **Error Handling Strategy:** A unified `BlennyError` type and anti‑fragile middleware are now in place.
- **Static Assets Clarification:** Conduit is for templates only; a separate `StaticAssets` component handles CSS/JS (implemented with hot‑reload and embedding).
- **Datastar Simplification:** When the Datastar encoder is active, connection‑intent filtering moves from the server to the client, eliminating the need for the `?intent=` query parameter on that endpoint.
- **WebSocket Configurability:** The WebSocket sidecar is toggled via `websocket: true` in the configuration; when disabled, only SSE is available, preserving the original Blenny’s optional‑transport design.
```
