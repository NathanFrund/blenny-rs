# Smalltalk Backport Tracker

Rust improvements that should be evaluated for inclusion in the Pharo Blenny reference implementation.

| # | Feature / Fix | Rust commit | Smalltalk status |
|---|---|---|---|
| 1 | `broadcast_data` should tag messages with category `"data"`, not `"ui"` | (pending) | ⬜ needs verification |
| 2 | SSE handler must merge personal user channel (per‑user direct messages) | (pending) | ⬜ needs verification |
| 3 | `public_routes()` mechanism for per‑route auth bypass | (existing) | ⬜ not implemented |
| 4 | Anti‑fragile middleware (panic‑catching, `BlennyError` JSON envelope) | (existing) | ⬜ not implemented |
| 5 | Multi‑layer config merging (env vars, JSON file, defaults) | (existing) | ⬜ not implemented |
| 6 | Transport auth required by default (`transport_auth_required` flag) | (existing) | ⬜ not implemented |

---

## Legend
- ⬜ not yet evaluated
- 🟡 planned
- ✅ implemented
- ❌ rejected (with reason)
