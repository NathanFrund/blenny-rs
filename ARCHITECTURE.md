Here is the complete updated `ARCHITECTURE.md` as a single markdown file. Copy it entirely and replace your current document.

```markdown
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
9. **Connection Intents (Message Filters)** – Every real‑time connection subscribes to one or more **intents** (`ui`, `command`, `notification`, `all`) via a `?intent=` query parameter. Publishers tag messages with a category, and the transport layer filters delivery, so clients only receive what they asked for.
10. **Pluggable Transport Encoders** – The SSE/WS transport layer can be configured to use different wire formats (e.g., Blenny’s standard JSON envelope, Datastar) without changing module code.
11. **Multi‑Layer Configuration** – Settings are merged from command‑line arguments, environment variables, a JSON file, and embedded defaults, in that priority order. Only overrides need to be specified.
12. **Anti‑Fragile Middleware** – Every handler response is wrapped by default to prevent server‑side crashes and enforce a consistent shape.
13. **Module Lifecycle & Control** – Modules have `initializeModule` (after injection, before routes), `startModule`, `stopModule`, and can be disabled via a simple flag without removing code.
14. **Template Ownership** – Modules tell their handlers which templates to use, keeping core handlers template‑agnostic.

## 🦀 Technical Architecture

### Workspace Layout
```

blenny-rs/
├── Cargo.toml # Workspace definition
├── blenny/ # Main crate (lib + binary)
│ ├── Cargo.toml
│ ├── src/
│ │ ├── main.rs # Entry point
│ │ ├── lib.rs # Framework core (traits, Conduit, builder)
│ │ ├── modules/
│ │ │ ├── mod.rs
│ │ │ ├── auth.rs # Example module (auto‑discovered)
│ │ │ └── dashboard.rs
│ ├── templates/ # Tera templates (hot‑reload in dev)
│ └── static/ # (future) CSS, JS, images
├── blenny-macros/ # Proc‑macro crate
│ ├── Cargo.toml
│ └── src/lib.rs
└── app/ # (optional separate binary)

````

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
- **Injection:** Conduit is stored as `Arc<Conduit>` and injected into the Axum router as an `Extension` (to be bundled into `AppState` soon). **Critical:** the `.layer(Extension)` must be applied *after* all routes are registered.
- **Handler usage:** `Extension(conduit): Extension<Arc<Conduit>>` extracts it.
- **Future:** Static assets (CSS, JS, images) will be managed by a separate `StaticAssets` component that uses the same dev/prod switch (hot‑reload from disk, embedded in binary) and auto‑mounts a `/static/*` route. Conduit is only for templates.

### TransportHub (Message Bus & Real‑Time)

- The `TransportHub` holds a Tokio broadcast channel for real‑time server‑to‑client events (SSE/WS), and it also serves as an **internal message bus**.
- **Connection Intents:** When a client connects to `/sse?intent=ui,notification`, the hub registers a filter. Only messages tagged with that category are delivered. Multiple intents can be combined (comma‑separated). A message tagged with `"ui"` will reach a client subscribed to `"ui"`, but not one subscribed only to `"notification"`.
- **Pluggable Encoders:** The SSE/WS bridge can be configured to use a `BlennyStandardEncoder` or a `DatastarEncoder`, changing the wire format without affecting modules.
- **Direct Per‑User Messaging:** The hub provides `direct_message(user_id, payload)` – messages are only sent to the specific user’s connections (after authentication).
- **Topic‑Based Pub/Sub (high priority):** Modules can publish to named topics (`"order.created"`) and subscribe to them, decoupling inter‑module communication. This will be implemented as a `HashMap<String, broadcast::Sender<Vec<u8>>>` inside `TransportHub` right after authentication is stable.
- **Backpressure:** Tokio’s broadcast channel drops messages for slow consumers. The buffer size is currently fixed (256); future enhancement will make it configurable and log warnings on drops.
- The API will eventually mirror the Smalltalk `BlennyPublisher` with methods like `broadcast_html(html)`, `broadcast_data(data)`, `direct_html(user_id, html)`, and topic‑specific publish/subscribe.
- Currently, the hub provides `broadcast_html()` and `broadcast_data()` for global client broadcasts, and SSE endpoints are auto‑mounted.

### Configuration System

- Configuration is built from four layers, merged in priority order:
  1. Command‑line arguments (`--key=value`)
  2. Environment variables (`BLENNY_*`)
  3. `blenny.json` file (or application‑specific JSON)
  4. Embedded defaults
- Only overrides need to be specified; every key has a sensible default.
- In Rust, this can be implemented with the `figment` or `config` crate.
- Currently, only the port and template directory are configurable; full layered config is planned.

### Middleware (Anti‑Fragile)

- All module route handlers are automatically wrapped to ensure:
  - Responses are shaped consistently (e.g., `{ "status": "ok", "data": ... }`).
  - Exceptions are caught and turned into graceful HTTP 500 responses instead of crashing the server.
- Infrastructure routes (`/sse`, `/ws`, `/static`) bypass the global middleware stack; authentication for these is handled inside the endpoint itself.
- This is implemented in Rust with Tower layers (`ServiceBuilder`) that wrap the module router.

### Auth System (Pluggable)

- **Trait:** `AuthProvider` – provides `auth_routes()` (login/logout endpoints) and `protect_layer()` (a Tower layer that validates JWT and injects a `User` extension).
- **Auto‑Discovery:** A second proc‑macro `#[blenny_auth_provider]` registers an `AuthRegistration` in the inventory. The builder picks the first one and uses it.
- **Middleware Layer Cake:** The protect layer is applied **before** auth routes are merged, so login is public; module routes are behind the guard.
- **HTMX‑Aware Unauthorized Response:** When an unauthenticated request bears an `HX-Request` header, the middleware returns `401` with `HX-Redirect: /login`, causing HTMX to seamlessly redirect to the login page.
- **User Injection:** Handlers extract `Extension<User>` to know who is logged in.
- **Future finer‑grained auth:** Modules will be able to mark individual routes as public via a `#[public]` attribute or a per‑route guard, allowing a mix of public and protected endpoints within the same module.
- **JWT refresh / sliding windows** are not yet implemented; the current system uses time‑based expiry. This can be added as a module‑level enhancement.

### Infrastructure vs. Module Routes

- Routes added by modules (`BlennyModule::register_routes`) go through the full middleware pipeline (including auth security layer).
- Framework‑added endpoints (`/health`, `/sse`, `/ws/*`, `/static/*`) are mounted outside that pipeline, so they remain accessible even when auth is enabled. Authentication for these (e.g., requiring a token on `/sse`) is implemented via query parameters or custom logic within the endpoint itself.

### Module Lifecycle

1. **Discover** – `BlennyBuilder` iterates over the inventory; skips disabled modules.
2. **Instantiate** – Constructor called; default injectable services (conduit, hub, auth) are set via setter traits or the bundled `AppState` (future).
3. **Initialize** – `initialize_module()` called on the module. This is where handlers are configured, template paths assigned, etc.
4. **Register Routes** – Each module’s `register_routes(router)` is called.
5. **Start** – After routes are assembled and the server is about to listen, `start_module()` is called on every module (for background tasks, etc.).
6. **Stop** – On graceful shutdown, `stop_module()` is called in reverse order.
- **Robustness:** If `start_module` panics, the error is logged and the server continues (the module stays disabled). Shutdown timeout will be configurable (e.g., modules have 5s to stop gracefully, then are forced).

### Service Bundle (AppState)

- To prevent fragmented `Extension<T>` layers and ease a future migration to `State`, all singletons will be bundled into a single `AppState` struct:
  ```rust
  pub struct AppState {
      pub conduit: Option<Arc<Conduit>>,
      pub hub: Arc<TransportHub>,
      pub auth: Option<Arc<dyn AuthProvider>>,
  }
````

- This will be injected as `Extension<AppState>` (later `State<AppState>`). Handlers can extract the whole state or individual fields.
- This refactor will happen right after authentication is stable.

### Error Handling Strategy (Planned)

- A `BlennyError` enum (using `thiserror`) will provide a unified error type for handlers.
- The anti‑fragile middleware will map `BlennyError` variants to appropriate HTTP responses (4xx/5xx) and log them.
- For now, handlers return `Result` with `unwrap_or_else` for simplicity.

### Static Assets (Future)

- CSS, JS, images will be managed by a `StaticAssets` component, separate from Conduit.
- It will use the same dev/prod switch: hot‑reload from a `static/` directory in debug; embed with `rust-embed` in release.
- A `/static/*` route will be auto‑mounted, reading from the embedded or hot‑reload source.
- This ensures a single‑binary deployment with no external file dependencies.

### Dependency Stack (key crates)

- **axum** + **tokio** – async web server
- **tower‑http** – middleware (CORS, tracing, auth)
- **tera** – template engine
- **inventory** – compile‑time module registration
- **rust‑embed** – embed templates / static files
- **notify** – hot‑reload file watcher
- **jsonwebtoken** – JWT authentication
- **serde** / **serde_json** – serialization
- **chrono** – timestamps for JWT
- **figment** (planned) – multi‑layer configuration
- **thiserror** (planned) – error handling

## 🧭 Roadmap & Implementation Status

| Feature                                                  | Status            |
| -------------------------------------------------------- | ----------------- |
| Self‑assembling modules (`#[blenny_module]`)             | ✅ Implemented    |
| Conduit (Tera) rendering with hot‑reload                 | ✅ Implemented    |
| Frozen production mode (embed templates)                 | ✅ Implemented    |
| Real‑time SSE broadcast (`TransportHub`)                 | ✅ Implemented    |
| Module life‑cycle hooks (`start/stop`)                   | 🟡 Partial        |
| Pluggable authentication (JWT)                           | 🔨 In progress    |
| Service bundle (`AppState`)                              | 🔜 After auth     |
| Topic‑based pub/sub for inter‑module messaging           | 🔜 After AppState |
| Connection intents (message filtering)                   | ⬜ Planned        |
| Pluggable transport encoders                             | ⬜ Planned        |
| Multi‑layer configuration                                | ⬜ Planned        |
| Anti‑fragile middleware                                  | ⬜ Planned        |
| Direct per‑user messaging                                | ⬜ Planned        |
| Per‑route auth control (`#[public]` attribute)           | ⬜ Planned        |
| WebSocket sidecar                                        | ⬜ Planned        |
| Static asset management (CSS, JS)                        | ⬜ Planned        |
| Unified error handling (`BlennyError`)                   | ⬜ Planned        |
| SurrealDB integration                                    | ⬜ Planned        |
| Dev‑friendly proc‑macro improvements (path prefix, etc.) | ⬜ Planned        |

## 📝 Key Architectural Decisions

| Decision                                           | Rationale                                                                                                       |
| -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| **`inventory` over `linkme`**                      | Eliminates false rust‑analyzer errors, better DX. Same compile‑time discovery.                                  |
| **`Arc<Conduit>` as `Extension` → later `State`**  | Shared, cheap to clone. Bundled into `AppState` soon for a single injection point.                              |
| **Layers applied after routes**                    | Axum requires that extensions/middleware wrap all routes; adding after route registration ensures availability. |
| **Conduit strips extensions**                      | Keeps module template names clean (`"auth/login"`) while files stay `.tera`.                                    |
| **Proc‑macro in separate crate**                   | Required by Rust for proc‑macros. Re‑exported with `pub use blenny_macros::blenny_module`.                      |
| **`extern crate self as blenny`**                  | Allows generated code to refer to `blenny::ModuleRegistration` from inside the crate. Do not remove.            |
| **Auth protect layer before auth routes**          | Keeps login routes public while protecting module routes.                                                       |
| **Infrastructure routes bypass global middleware** | Avoids double authentication and keeps health/SSE/static endpoints simple.                                      |
| **Connection intents via query params**            | Mirrors the Smalltalk model; lightweight and easy to implement with Tokio channels.                             |
| **Pluggable encoders as configuration**            | Allows swapping wire format (Standard vs Datastar) without code changes.                                        |
| **Feature flags for dev/prod**                     | `#[cfg(debug_assertions)]` currently used; will be replaced by a `hot-reload` feature for finer control.        |
| **Topic‑based pub/sub prioritized**                | Unlocks decoupled inter‑module communication with minimal API surface.                                          |

## 🧘‍♀️ Philosophy Summary

Blenny‑rs is not a direct copy of the Smalltalk implementation; it’s a re‑expression of Blenny’s principles using Rust’s strengths: compile‑time safety, zero‑cost abstractions, and a rich type system. The result is a framework where you drop a struct and it becomes a living part of the server – exactly the magic that made the original Blenny special. Every design choice, from connection intents to pluggable transport encoders, is aimed at preserving the friction‑free, “just works” developer experience while embracing Rust’s idioms.

## 📋 Post‑Review Refinements

- **Bundled State:** All singletons will be grouped into an `AppState` struct to simplify injection and future `State` migration.
- **Module Lifecycle Robustness:** Shutdown timeouts, panic handling, and clear ordering guarantees added to the lifecycle specification.
- **Fine‑Grained Auth:** Per‑route access control (public/private) is now on the roadmap.
- **Topic‑Based Pub/Sub Prioritized:** Moved up in priority to unlock inter‑module patterns early.
- **Backpressure Awareness:** Documented the broadcast buffer behavior and future configurability.
- **Error Handling Strategy:** Planned a unified `BlennyError` type.
- **Static Assets Clarification:** Conduit is for templates only; a separate `StaticAssets` component will handle CSS/JS.

```

```
