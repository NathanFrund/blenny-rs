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
5. **Broadcasting from Anywhere** – A `TransportHub` (future) allows any code to push server‑sent events.
6. **Pluggable Auth** – A module can _become_ the auth UI and logic; swapping it requires no rewiring.
7. **Minimal Ceremony** – The entry point (`main.rs`) is 5 lines. No Makefile codegen, no registry files.

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
│   │   │   └── auth.rs # Example module
│   ├── templates/      # Tera templates (hot‑reload in dev)
│   └── static/         # (future) CSS, JS
├── blenny-macros/      # Proc‑macro crate
│   ├── Cargo.toml
│   └── src/lib.rs
└── app/                # (optional separate binary)
```

### Module System (Self‑Assembly)

- **Trait:** `BlennyModule` (in `blenny::`) – provides `name()` and `register_routes(router) -> Router`.
- **Proc Macro:** `#[blenny_module]` generates an `inventory::submit!` that pushes a `ModuleRegistration` (name + constructor) into a global collector.
- **Registry:** `inventory::collect!(ModuleRegistration)` gathers all submissions at compile time, replacing the earlier `linkme` approach (which caused false rust‑analyzer errors).
- **Builder:** `BlennyBuilder::serve()` iterates over `inventory::iter::<ModuleRegistration>`, constructs each module, calls `register_routes`, and merges them into the Axum router.
- **`extern crate self as blenny;`** added to `lib.rs` so proc‑macro generated code (`blenny::ModuleRegistration`) works inside the crate.

### Conduit (Template Engine)

- Wraps a `tera::Tera` instance.
- **Modes:** `hot_reload(path)` (dev) reads from disk; `frozen()` (prod) will embed templates via `rust-embed`.
- **Extension stripping:** `render()` automatically appends `.tera` if the caller omits it, allowing modules to request `"auth/login"` while files are `auth/login.tera`.
- **Injection:** Conduit is stored as `Arc<Conduit>` and injected into the Axum router as an `Extension` (to be upgraded to `State` later). **Critical:** the `.layer(Extension)` must be applied _after_ all routes are registered (lesson learned).
- **Handler usage:** `Extension(conduit): Extension<Arc<Conduit>>` extracts it.

### Builder & Server Startup

- `BlennyBuilder::default()` starts with nothing.
- `.with_conduit(conduit)` attaches the template engine.
- `.with_default_transports()` (placeholder) will add SSE/WebSocket.
- `.serve(addr)`:
  1. Auto‑discovers modules via `inventory`.
  2. Registers routes from each module.
  3. Adds core routes (e.g., `/health`).
  4. Applies layers (Conduit extension, middleware) **last**.
  5. Binds and serves with graceful shutdown.

### Dependency Stack (key crates)

- **axum** + **tokio** – async web server
- **tower-http** – middleware (CORS, tracing)
- **tera** – template engine
- **inventory** – compile‑time module registration
- **syn**, **quote**, **proc-macro2** – for the proc‑macro
- **rust-embed** (future) – embed static assets
- **notify** (future) – hot‑reload file watcher
- **jsonwebtoken** (future) – JWT auth

## ✅ Current State (Baseline Commit)

- Workspace compiles and runs.
- `AuthModule` (in `modules/auth.rs`) is auto‑discovered and serves `/login`.
- `/login` renders the Tera template `templates/auth/login.tera` (hot‑reload from disk).
- No false lint errors (switched from `linkme` to `inventory`).
- Extension injection ordering corrected (layers after routes).
- `main.rs` is minimal: just creates Conduit, calls builder.

## 🧭 Next Steps (Ordered)

1. **Hot‑Reload File Watcher** – Use `notify` to automatically reload Tera when templates change, emulating Smalltalk’s live feedback.
2. **Frozen Mode** – Implement `Conduit::frozen()` using `rust-embed` (`include_dir!` alternative) so a `--release` build contains all templates.
3. **Real‑Time Transports** – Add SSE bridge (`tokio::sync::broadcast`) and optional WebSocket. Expose a `TransportHub` that modules can extract.
4. **Auth System** – Allow modules to implement an `AuthProvider` trait; auto‑detect and apply authentication middleware.
5. **HTMX‑Aware Rendering** – Conduit automatically wraps fragment responses in a layout when `HX-Request` header is absent.
6. **Static Assets** – Unify static file serving under Conduit (hot‑reload CSS/JS in dev, embed in prod).
7. **Dependency Injection** – Replace `Extension` with `State` for stronger guarantees, and add optional setter traits (`HasAuthProvider`, etc.) for modules that need extra services.
8. **SurrealDB Integration** – Provide a native SurrealDB client as an extension for modules.
9. **Developer Experience** – Improve the proc‑macro to allow path attributes (e.g., `#[blenny_module(path = "/auth")]`) for route prefixing.

## 📝 Key Architectural Decisions

| Decision                                          | Rationale                                                                                                                |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| **`inventory` over `linkme`**                     | Eliminates false rust‑analyzer errors, better DX. Same compile‑time discovery.                                           |
| **`Arc<Conduit>` as `Extension` (later `State`)** | Shared, cheap to clone, and allows future swap to `State`.                                                               |
| **Layers applied last**                           | Axum requires that extensions/middleware wrap all routes; adding after route registration ensures availability.          |
| **Conduit strips extensions**                     | Keeps module template names clean (`"auth/login"`) while files stay `.tera`.                                             |
| **Proc‑macro in separate crate**                  | Required by Rust for proc‑macros. Re‑exported with `pub use blenny_macros::blenny_module`.                               |
| **`extern crate self as blenny`**                 | Allows generated code to refer to `blenny::ModuleRegistration` from inside the crate.                                    |
| **Future `LazyLock`**                             | When global singletons like a `TransportHub` are needed, `LazyLock` (Rust 1.80+) will provide lazy, safe initialization. |
| **Feature flags for dev/prod**                    | `#[cfg(debug_assertions)]` currently used; will be replaced by a `hot-reload` feature for finer control.                 |

## 🧘‍♀️ Philosophy Summary

Blenny‑rs is not a direct copy of the Smalltalk implementation; it’s a re‑expression of Blenny’s principles using Rust’s strengths: compile‑time safety, zero‑cost abstractions, and a rich type system. The result is a framework where you drop a struct and it becomes a living part of the server – exactly the magic that made the original Blenny special.
