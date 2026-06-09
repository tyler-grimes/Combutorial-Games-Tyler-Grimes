# Connect-4 LAN Server

Locally hosted Connect-4: players on the same network play in the browser,
or face a perfect-play bot backed by a from-scratch Rust solver.

## Quick start

```bash
# run the server (from this directory)
cargo run -p server --release
```

Players open `http://<your-LAN-IP>:3000` in any browser. Find your IP with
`ip addr` (Linux) / `ipconfig` (Windows).

## Bot

The bot uses time-limited iterative-deepening alpha-beta (default ~2s/move). It
solves exactly when the tree is shallow enough (mid/late game) and plays a strong
heuristic move otherwise (opening) — so it never stalls and never needs a book.

Optional: drop an opening book at `engine/book.bin` for provably-perfect opening
play. The engine loads it if present; positions whose children are fully covered
are answered by instant exact lookup instead of search.

```bash
# generate a book (slow near the opening; higher depth = more perfect coverage)
cargo run -p engine --release --bin book_gen -- 8 engine/book.bin
```

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
