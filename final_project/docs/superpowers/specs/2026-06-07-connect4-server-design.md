# Connect-4 LAN Server with Perfect-Play Bot — Design

**Date:** 2026-06-07
**Status:** Approved

## Goal

A locally hosted Connect-4 server in Rust. Players on the same network join via
browser and play each other, or play against a bot backed by a perfect-play
solver. The bot never misplays.

## Decisions Made

| Question | Decision |
|---|---|
| Client interface | Browser web UI served by the server (WebSockets for live play) |
| Engine | Write our own solver in Rust (bitboards, negamax). `connect-four-ai` crate used only as a test oracle |
| Early-game speed | Offline-generated opening book (depth ~10) loaded at server start |
| Lobby model | Rooms + lobby; many concurrent games, join via code or click |
| Extras in scope | Reconnect support, spectators, engine analysis display, rematch |
| Stack | Approach A: axum + WebSockets + vanilla JS frontend (cargo workspace) |

## Architecture

```
final_project/                  (cargo workspace)
├── engine/                     # pure Rust lib, no async, no I/O deps
│   ├── src/
│   │   ├── lib.rs
│   │   ├── board.rs            # bitboard position (2x u64)
│   │   ├── solver.rs           # negamax + alpha-beta + transposition table
│   │   ├── book.rs             # opening book: load, lookup
│   │   └── bin/book_gen.rs     # offline book generator
│   └── book.bin                # generated artifact, committed to git (few MB)
├── server/
│   ├── src/
│   │   ├── main.rs             # axum setup, routes
│   │   ├── state.rs            # AppState: rooms map, engine handle
│   │   ├── room.rs             # Room: game state, players, broadcast channel
│   │   ├── ws.rs               # WebSocket handler, message dispatch
│   │   ├── bot.rs              # bot move task, calls engine
│   │   └── protocol.rs         # serde message types (client <-> server)
│   └── static/
│       ├── index.html          # lobby + board, single page
│       ├── app.js
│       └── style.css
└── Cargo.toml                  # workspace root
```

Flow: server binds `0.0.0.0:3000` → LAN players open `http://<host-ip>:3000` →
page opens a WebSocket to `/ws` → all lobby and game actions travel over the
WebSocket as JSON.

Engine boundary: `solve(position) -> Score`, `best_move(position) -> column`.
The server never touches bitboards directly; bot moves call the engine inside
`tokio::task::spawn_blocking` so the async runtime is never stalled.

## Engine

**Bitboard** (Pascal Pons layout): 7 columns × 6 rows plus a sentinel row =
49 bits in a `u64`. Two fields: `position` (current player's stones) and
`mask` (all stones). Win detection is 4 shift-AND operations per direction;
playing a move is a single bit addition.

**Solver:**
- Negamax with exact scores: `(43 - moves_when_win) / 2`, sign-flipped per
  side — gives distance-to-win, not just win/loss/draw.
- Alpha-beta with null-window search (fail-soft), iteratively narrowing the
  score window.
- Transposition table: fixed-size array (~64 MB), key = position hash, stores
  upper/lower bounds.
- Move ordering: center-first static order plus best-move hints from the
  transposition table.
- Early exits: immediate-win detection and opponent-threat forced moves
  before recursing.

**Opening book:**
- `book_gen` binary enumerates all positions to depth N (default 10), solves
  each, and writes `book.bin` — sorted key→score pairs, binary-search lookup.
- Mirror-symmetric positions are deduplicated (halves the book).
- Server loads the book at startup. `best_move` consults the book first and
  falls back to search. With a depth-10 book, runtime searches start at move
  11 (≤32 plies remaining): typically <1 s, worst case a few seconds.
- One-time generation is expected to be long (hours); run offline/overnight.

**Perfect play guarantee:** `best_move` returns the argmax of solved scores
over all legal moves. Tie-break: fastest win / slowest loss. Perfect by
construction — every move is backed by an exact solve.

**Oracle verification:** dev-dependency on the `connect-four-ai` crate
(independent perfect solver, MIT): random playouts comparing scores and move
choices. Also the published gamesolver.org test sets (6000 positions with
known exact scores).

## Server, Protocol, Data Flow

**State:**

```rust
AppState {
    rooms: Mutex<HashMap<RoomId, Room>>,   // RoomId = 4-char code
    engine: Arc<Engine>,                   // solver + book, shared
}
Room {
    game: Game,                            // board, turn, status
    players: [Option<PlayerSlot>; 2],      // session token + name + connected flag
    spectator_count: usize,
    tx: broadcast::Sender<ServerMsg>,      // fan-out to all room sockets
    bot: Option<BotSide>,
}
```

**Sessions and reconnect:** on join the server issues a UUID session token,
stored by the client in `localStorage`. A WebSocket drop marks the slot
disconnected but the game persists. Rejoining with the token reclaims the
slot and receives a full state snapshot. Rooms are reaped after 1 hour of
inactivity.

**Protocol** (JSON over WebSocket, serde tagged enums):

- Client → Server: `CreateRoom{name, vs_bot, bot_first?}`,
  `JoinRoom{code, name, token?}`, `ListRooms`, `Move{col}`, `Rematch`,
  `Spectate{code}`
- Server → Client: `RoomList{..}`, `Joined{token, color, state}`,
  `State{board, turn, status}` (full snapshot on every change — simple, no
  delta bugs), `MovePlayed{col, by, eval?}`, `GameOver{winner, line}`,
  `Error{msg}`

**Bot integration:** the bot is not a separate connection. After each human
move in a bot room, the server runs the engine in `spawn_blocking`, applies
the bot's move, and broadcasts it. In bot-first games the bot moves
immediately on room creation.

**Analysis display:** every `MovePlayed` carries an `eval` field (solved
score plus outcome text, e.g. "P1 wins in 9"). The frontend has a toggle to
show or hide it. Spectators can always view it.

**Rematch:** both players (or the single human in a bot game) send `Rematch`
→ board resets and the starting player swaps.

## Frontend

Vanilla JS, single page, two views:

- **Lobby:** name input, live room list (pushed over WS), create room
  (vs human / vs bot, choose who starts), join by code.
- **Game:** 7×6 CSS grid, click a column to drop, hover preview, turn
  indicator, eval toggle, rematch button, winning-line highlight.
- WebSocket auto-reconnects with the stored token after a disconnect.

## Error Handling

- Illegal move (full column, not your turn): server replies `Error`, state
  unchanged. Server is the authority; the client is never trusted.
- Malformed JSON: close the socket.
- Room full / unknown code: `Error` shown in lobby.
- Engine panic (should not happen): caught via the `spawn_blocking`
  `JoinError`; the bot game forfeits gracefully instead of hanging.
- Two tabs with the same token: newest socket wins; the old one is dropped.

## Testing

- **Engine unit tests:** win detection, move generation, symmetry, board
  edge cases.
- **Engine oracle tests:** gamesolver.org test sets and `connect-four-ai`
  cross-checks. Slow/deep tiers marked `#[ignore]`, run explicitly.
- **Server tests:** serde round-trips for every protocol message;
  integration tests with a `tokio-tungstenite` client covering
  create/join/play/win, the reconnect flow, and illegal-move rejection.
- **Bot end-to-end:** bot vs bot full game must end in a first-player win
  (known Connect-4 theory) — a strong perfect-play smoke test.
- **Manual:** two browsers on the LAN.
