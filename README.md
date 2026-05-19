# Blenny‑rs

<p align="center">
  <img src="blenny-rs-logo.svg" alt="Blenny Mascot" width="250">
</p>

Blenny‑rs is a real‑time web framework that ports the philosophy of
[Pharo Smalltalk Blenny](https://github.com/NathanFrund/blenny) to idiomatic,
compile‑safe Rust. Modules self‑assemble via proc‑macros, real‑time is default
(SSE + optional WebSocket), and deployment is a single binary.

---

## Features

- **Self‑Assembling Modules** – `#[blenny_module]` on a struct; no manual wiring.
- **Real‑Time by Default** – SSE always on; WebSocket opt‑in. Global broadcast,
  per‑user direct messaging, and topic‑based pub/sub in one hub.
- **Unified Rendering (Conduit)** – Templates by name (`"auth/login"`); hot‑reload
  in dev, embedded in prod for a single binary.
- **Pluggable Auth** – Drop in a module, declare public paths, JWT‑based.
- **Connection Intents** – `?intent=ui,notification` filters messages server‑side
  (standard) or client‑side (Datastar).
- **Anti‑Fragile Middleware** – Catches panics, returns structured JSON errors.
- **Multi‑Layer Config** – Env vars, JSON file, sensible defaults via `figment`.
- **Optional SurrealDB** – Native async client, feature‑gated.
- **Single Binary** – Templates & static assets embedded in release builds.

---

## Quick Start

```bash
cargo new my-app && cd my-app
cargo add blenny
```

```rust
use blenny::BlennyBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    BlennyBuilder::default()
        .serve("0.0.0.0:8081")
        .await
}
```

Then open `http://localhost:8081/login` – you have a live, real‑time server with
auth, SSE, and template rendering. Add your own module:

```rust
use axum::Router;
use blenny::{blenny_module, BlennyModule};

#[derive(Default)]
#[blenny_module(route_handler = "dashboard_routes")]
pub struct DashboardModule;

fn dashboard_routes(router: Router) -> Router {
    router.route("/", axum::routing::get(|| async { "Hello, Blenny!" }))
}
```

No registration, no `main.rs` edits. Just save and restart.

---

## Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)** – design blueprint, roadmap, key decisions.
- **[SMALLTALK_BACKPORT.md](SMALLTALK_BACKPORT.md)** – tracking improvements ported back to Smalltalk.
- **[agents.md](agents.md)** – briefing for AI assistants / new contributors.

---

## Running Tests

```bash
# All tests
cargo test -p blenny

# With Datastar encoder
cargo test -p blenny --features datastar-sse

# With SurrealDB (requires a running instance)
SURREALDB_URL=ws://localhost:8000 cargo test -p blenny --features surreal
```

---

## Feature Flags

| Flag           | Enables                       |
| -------------- | ----------------------------- |
| `datastar-sse` | Official Datastar SSE encoder |
| `surreal`      | SurrealDB client connection   |

---

## Supported Encoders

- **Standard** – JSON‑enveloped SSE (default).
- **Datastar** – Official SDK, native event listeners (`datastar-patch-elements`, etc.).

---

## Philosophy

Blenny‑rs is a re‑expression of the Smalltalk Blenny principles in Rust:
compile‑time safety, zero‑cost abstractions, and a rich type system.
Every design choice—from connection intents to pluggable transport encoders—aims
to preserve the friction‑free, “just works” developer experience.

---

## License

MIT
