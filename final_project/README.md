# Connect-4 LAN Server

Locally hosted Connect-4: players on the same network play in the browser,
or face a perfect-play bot backed by a from-scratch Rust solver.

## Quick start

```bash
# one-time: generate the opening book (deeper = snappier bot; 8 is a good target)
cargo run -p engine --release --bin book_gen -- 8 engine/book.bin

# run the server (from this directory)
cargo run -p server --release
```

Players open `http://<your-LAN-IP>:3000` in any browser. Find your IP with
`ip addr` (Linux) / `ipconfig` (Windows).

## Layout

- `engine/` — bitboard solver: negamax + alpha-beta + transposition table +
  opening book. Pure Rust, no async. See the design spec in
  `docs/superpowers/specs/2026-06-07-connect4-server-design.md`.
- `server/` — axum WebSocket server: lobby, rooms, spectators, reconnect,
  rematch, bot integration.
- `server/static/` — vanilla JS frontend.

## Tests

```bash
cargo test --workspace --release            # fast suite (incl. oracle cross-checks)
cargo test --workspace --release -- --ignored   # deep: empty-board solve, bot-vs-bot e2e
```
