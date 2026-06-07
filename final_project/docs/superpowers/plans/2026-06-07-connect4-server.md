# Connect-4 LAN Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Locally hosted Connect-4 server: LAN players join via browser; perfect-play bot backed by our own solver.

**Architecture:** Cargo workspace. `engine` = pure-Rust bitboard solver (negamax + alpha-beta + transposition table + opening book), no async/I-O deps. `server` = axum 0.8: static files + `/ws` WebSocket, rooms in `Mutex<HashMap>`, per-room `tokio::broadcast`, bot calls engine via `spawn_blocking`. Frontend = one static HTML/JS/CSS page.

**Tech Stack:** Rust, axum 0.8, tokio 1, tower-http 0.6 (ServeDir), serde/serde_json 1, uuid 1, tokio-tungstenite 0.29 (tests only), connect-four-ai 1.0 (dev-dep test oracle).

**Spec:** `docs/superpowers/specs/2026-06-07-connect4-server-design.md`

**Conventions used throughout:**
- All commands run from `final_project/`.
- Score convention (Pascal Pons): positive = player-to-move wins; score = `(43 - move_number_of_win) / 2`; negative mirror for losses; 0 = draw.
- Columns 0-indexed internally; `from_moves("4453...")` strings are 1-indexed (matches gamesolver.org + oracle crate).
- Bitboard layout: bit `col*7 + row`, row 0 = bottom, 7th bit of each column = sentinel.

---

### Task 1: Workspace scaffolding

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `engine/Cargo.toml`, `engine/src/lib.rs`
- Create: `server/Cargo.toml`, `server/src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: Create workspace root `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["engine", "server"]
```

- [ ] **Step 2: Create `engine/Cargo.toml`**

```toml
[package]
name = "engine"
version = "0.1.0"
edition = "2021"

[dev-dependencies]
connect-four-ai = "1.0.0"
rand = "0.10"

[[bin]]
name = "book_gen"
path = "src/bin/book_gen.rs"
```

- [ ] **Step 3: Create `engine/src/lib.rs` placeholder module decls**

```rust
pub mod board;
pub mod solver;
pub mod book;

pub use board::Position;
pub use solver::Solver;
pub use book::Book;
```

Also create empty files `engine/src/board.rs`, `engine/src/solver.rs`, `engine/src/book.rs`, and `engine/src/bin/book_gen.rs` containing `fn main() {}` so the workspace compiles.

- [ ] **Step 4: Create `server/Cargo.toml`**

```toml
[package]
name = "server"
version = "0.1.0"
edition = "2021"

[dependencies]
engine = { path = "../engine" }
axum = { version = "0.8", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["fs"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
rand = "0.10"

[dev-dependencies]
tokio-tungstenite = "0.29"
futures-util = "0.3"
```

- [ ] **Step 5: Create `server/src/main.rs` placeholder**

```rust
fn main() {
    println!("server placeholder");
}
```

- [ ] **Step 6: Create `.gitignore`**

```
target/
```

- [ ] **Step 7: Verify workspace builds**

Run: `cargo build --workspace`
Expected: compiles, warnings ok (empty modules).

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: scaffold connect4 cargo workspace"
```

---

### Task 2: Engine — bitboard Position basics

**Files:**
- Modify: `engine/src/board.rs`
- Tests: inline `#[cfg(test)]` module in same file

Bitboard (Pons layout): `WIDTH=7`, `HEIGHT=6`, column stride `H1 = HEIGHT+1 = 7` bits (top bit of each column = sentinel). `current` = stones of player to move, `mask` = all stones. `key = current + mask` uniquely identifies a position.

- [ ] **Step 1: Write failing tests**

In `engine/src/board.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_position_is_empty() {
        let p = Position::new();
        assert_eq!(p.moves(), 0);
        for c in 0..WIDTH {
            assert!(p.can_play(c));
        }
    }

    #[test]
    fn column_fills_after_height_moves() {
        let mut p = Position::new();
        for _ in 0..HEIGHT {
            assert!(p.can_play(3));
            p.play(3);
        }
        assert!(!p.can_play(3));
        assert_eq!(p.moves(), HEIGHT as u32);
    }

    #[test]
    fn from_moves_parses_one_indexed() {
        // "44" = two stones in column index 3
        let p = Position::from_moves("44").unwrap();
        assert_eq!(p.moves(), 2);
        let g = p.to_grid();
        assert_eq!(g[0][3], 1); // bottom row, P1
        assert_eq!(g[1][3], 2); // second row, P2
        assert_eq!(g[2][3], 0);
    }

    #[test]
    fn from_moves_rejects_bad_input() {
        assert!(Position::from_moves("8").is_none());
        assert!(Position::from_moves("4444444").is_none()); // 7th into full col
    }

    #[test]
    fn keys_differ_between_positions() {
        let a = Position::from_moves("1").unwrap();
        let b = Position::from_moves("2").unwrap();
        assert_ne!(a.key(), b.key());
    }

    #[test]
    fn mirror_of_col0_equals_col6() {
        let a = Position::from_moves("1").unwrap();
        let c = Position::from_moves("7").unwrap();
        assert_eq!(a.canonical_key(), c.canonical_key());
        assert_ne!(a.key(), c.key());
    }
}
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p engine`
Expected: compile error (Position undefined) — counts as failing.

- [ ] **Step 3: Implement Position**

`engine/src/board.rs` (above the test module):

```rust
pub const WIDTH: usize = 7;
pub const HEIGHT: usize = 6;
const H1: usize = HEIGHT + 1; // bits per column incl. sentinel

const fn bottom() -> u64 {
    let mut b = 0u64;
    let mut c = 0;
    while c < WIDTH {
        b |= 1 << (c * H1);
        c += 1;
    }
    b
}
pub(crate) const BOTTOM_MASK: u64 = bottom();
pub(crate) const BOARD_MASK: u64 = BOTTOM_MASK * ((1 << HEIGHT) - 1);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position {
    current: u64, // stones of the player to move
    mask: u64,    // all stones
    moves: u32,
}

impl Position {
    pub fn new() -> Self {
        Position { current: 0, mask: 0, moves: 0 }
    }

    const fn bottom_mask_col(col: usize) -> u64 {
        1 << (col * H1)
    }
    const fn top_mask_col(col: usize) -> u64 {
        1 << (HEIGHT - 1 + col * H1)
    }
    pub(crate) const fn column_mask(col: usize) -> u64 {
        ((1 << HEIGHT) - 1) << (col * H1)
    }

    pub fn moves(&self) -> u32 {
        self.moves
    }

    pub fn can_play(&self, col: usize) -> bool {
        col < WIDTH && self.mask & Self::top_mask_col(col) == 0
    }

    /// Play in `col`. Caller must check `can_play`.
    pub fn play(&mut self, col: usize) {
        self.current ^= self.mask;
        self.mask |= self.mask + Self::bottom_mask_col(col);
        self.moves += 1;
    }

    /// 1-indexed move string, e.g. "4453". None on illegal/garbage.
    pub fn from_moves(s: &str) -> Option<Self> {
        let mut p = Position::new();
        for ch in s.chars() {
            let col = ch.to_digit(10)? as usize;
            if col < 1 || col > WIDTH || !p.can_play(col - 1) {
                return None;
            }
            p.play(col - 1);
        }
        Some(p)
    }

    /// Unique key for this position.
    pub fn key(&self) -> u64 {
        self.current + self.mask
    }

    fn mirrored(v: u64) -> u64 {
        let mut r = 0u64;
        for c in 0..WIDTH {
            let col_bits = (v >> (c * H1)) & ((1 << H1) - 1);
            r |= col_bits << ((WIDTH - 1 - c) * H1);
        }
        r
    }

    pub fn mirror_key(&self) -> u64 {
        Self::mirrored(self.current) + Self::mirrored(self.mask)
    }

    /// min(key, mirror_key) — same for a position and its mirror image.
    pub fn canonical_key(&self) -> u64 {
        self.key().min(self.mirror_key())
    }

    /// Grid for display: [row][col], row 0 = bottom. 0 empty, 1 = first player, 2 = second.
    pub fn to_grid(&self) -> [[u8; WIDTH]; HEIGHT] {
        let mut g = [[0u8; WIDTH]; HEIGHT];
        // `current` belongs to the player to move: P1 if moves is even.
        let (p1, p2) = if self.moves % 2 == 0 {
            (self.current, self.current ^ self.mask)
        } else {
            (self.current ^ self.mask, self.current)
        };
        for col in 0..WIDTH {
            for row in 0..HEIGHT {
                let bit = 1u64 << (col * H1 + row);
                if p1 & bit != 0 {
                    g[row][col] = 1;
                } else if p2 & bit != 0 {
                    g[row][col] = 2;
                }
            }
        }
        g
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p engine`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add engine/src/board.rs && git commit -m "feat(engine): bitboard Position with play/key/grid"
```

---

### Task 3: Engine — win detection

**Files:**
- Modify: `engine/src/board.rs`

Shift distances: vertical 1, horizontal 7 (`H1`), diagonal `/` 8 (`H1+1`), diagonal `\` 6 (`HEIGHT`).

- [ ] **Step 1: Write failing tests** (append inside `mod tests`)

```rust
    #[test]
    fn detects_vertical_win() {
        // P1: col 1 four times; P2: col 2 three times. "1212121" → P1 wins playing col 1? No —
        // build explicitly: moves 1,2,1,2,1,2 then col 1 is the winning move for P1.
        let p = Position::from_moves("121212").unwrap();
        assert!(p.is_winning_move(0));
        assert!(!p.is_winning_move(1));
    }

    #[test]
    fn detects_horizontal_win() {
        // P1 plays cols 1,2,3 bottom row; P2 stacks col 7.
        let p = Position::from_moves("172737").unwrap();
        assert!(p.is_winning_move(3)); // col 4 completes 1-2-3-4
        assert!(p.is_winning_move(4) == false);
    }

    #[test]
    fn detects_diagonal_win() {
        // Build / diagonal for P1: needs P1 at (c,r) = (0,0),(1,1),(2,2) and col 3
        // filled to height 3 so P1's drop lands at (3,3).
        // Move list (1-indexed cols), alternating P1,P2:
        // P1:1 → (0,0)   P2:2 → (1,0)
        // P1:2 → (1,1)   P2:3 → (2,0)
        // P1:7 → (6,0)   P2:3 → (2,1)
        // P1:3 → (2,2)   P2:4 → (3,0)
        // P1:7 → (6,1)   P2:4 → (3,1)
        // P1:7 → (6,2)   P2:4 → (3,2)
        // Now P1 to move; col 4 lands at (3,3): completes (0,0),(1,1),(2,2),(3,3).
        // P2 never has 4 anywhere (c3 stack is 3; scattered elsewhere).
        let p = Position::from_moves("122373377474").unwrap();
        assert_eq!(p.moves(), 12);
        assert!(p.is_winning_move(3));
        assert!(!p.is_winning_move(0));
    }
```

Sequence verification (alternating P1,P2 per char of "122373377474"): P1 gets 1→(0,0), 2→(1,1), 7→(6,0), 3→(2,2), 7→(6,1), 7→(6,2). P2 gets 2→(1,0), 3→(2,0), 3→(2,1), 4→(3,0), 4→(3,1), 4→(3,2). P1 holds (0,0),(1,1),(2,2); col 3 filled to height 3; P1 to move; P2 has no 4-line anywhere. Playing column index 3 lands at row 3 and completes the / diagonal.

- [ ] **Step 2: Run tests, verify fail**

Run: `cargo test -p engine detects`
Expected: compile error — `is_winning_move` undefined.

- [ ] **Step 3: Implement win detection** (append to `impl Position`)

```rust
    /// Bitmap of playable cells (lowest empty cell of each non-full column).
    pub(crate) fn possible(&self) -> u64 {
        (self.mask + BOTTOM_MASK) & BOARD_MASK
    }

    /// Open cells that would complete 4-in-a-row for `position` stones.
    pub(crate) fn compute_winning_position(position: u64, mask: u64) -> u64 {
        // vertical
        let mut r = (position << 1) & (position << 2) & (position << 3);
        // horizontal (shift H1)
        let mut p = (position << H1) & (position << (2 * H1));
        r |= p & (position << (3 * H1));
        r |= p & (position >> H1);
        p = (position >> H1) & (position >> (2 * H1));
        r |= p & (position << H1);
        r |= p & (position >> (3 * H1));
        // diagonal \ (shift HEIGHT)
        p = (position << HEIGHT) & (position << (2 * HEIGHT));
        r |= p & (position << (3 * HEIGHT));
        r |= p & (position >> HEIGHT);
        p = (position >> HEIGHT) & (position >> (2 * HEIGHT));
        r |= p & (position << HEIGHT);
        r |= p & (position >> (3 * HEIGHT));
        // diagonal / (shift HEIGHT+2 == H1+1)
        const S: usize = HEIGHT + 2;
        p = (position << S) & (position << (2 * S));
        r |= p & (position << (3 * S));
        r |= p & (position >> S);
        p = (position >> S) & (position >> (2 * S));
        r |= p & (position << S);
        r |= p & (position >> (3 * S));
        r & (BOARD_MASK ^ mask)
    }

    /// Cells where the player to move would win immediately.
    pub(crate) fn winning_position(&self) -> u64 {
        Self::compute_winning_position(self.current, self.mask)
    }

    /// Cells where the opponent would win immediately.
    pub(crate) fn opponent_winning_position(&self) -> u64 {
        Self::compute_winning_position(self.current ^ self.mask, self.mask)
    }

    pub fn is_winning_move(&self, col: usize) -> bool {
        self.winning_position() & self.possible() & Self::column_mask(col) != 0
    }

    /// True if the player to move can win this turn in some column.
    pub fn can_win_next(&self) -> bool {
        self.winning_position() & self.possible() != 0
    }
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p engine`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add engine/src/board.rs && git commit -m "feat(engine): bitboard win detection"
```

---

### Task 4: Engine — non-losing move filter + move count heuristic

**Files:**
- Modify: `engine/src/board.rs`

- [ ] **Step 1: Write failing tests** (append inside `mod tests`)

```rust
    #[test]
    fn forced_block_is_only_non_losing_move() {
        // P2 to move while P1 threatens vertical win in col 1.
        let p = Position::from_moves("12121").unwrap(); // P1 has 3 in col 0, P2 to move
        let nl = p.possible_non_losing_moves();
        // only legal cell: top of col 0 (block)
        assert_eq!(nl, nl & Position::column_mask(0));
        assert_ne!(nl, 0);
    }

    #[test]
    fn double_threat_means_no_non_losing_moves() {
        // P1 with open three 2,3,4 on bottom row: threats at cols 1 and 5.
        // P1: 2,3,4 bottom; P2: stacked col 7 ×3. P2 to move.
        let p = Position::from_moves("273747").unwrap();
        assert_eq!(p.possible_non_losing_moves(), 0);
    }

    #[test]
    fn move_score_counts_created_threats() {
        let p = Position::new();
        // any first move creates no immediate threats
        let m = Position::column_mask(3) & p.possible();
        assert_eq!(p.move_score(m), 0);
    }
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p engine non_losing`
Expected: compile error — methods undefined.

- [ ] **Step 3: Implement** (append to `impl Position`)

```rust
    /// Bitmap of moves that don't hand the opponent an immediate win.
    /// 0 means every move loses (or no moves). Assumes we cannot win this move.
    pub(crate) fn possible_non_losing_moves(&self) -> u64 {
        let mut possible = self.possible();
        let opponent_win = self.opponent_winning_position();
        let forced = possible & opponent_win;
        if forced != 0 {
            if forced & (forced - 1) != 0 {
                return 0; // two+ immediate threats: lost
            }
            possible = forced; // must block
        }
        // never play directly below an opponent winning cell
        possible & !(opponent_win >> 1)
    }

    /// Heuristic: number of winning cells we'd own after playing `move_bit`.
    pub(crate) fn move_score(&self, move_bit: u64) -> u32 {
        Self::compute_winning_position(self.current | move_bit, self.mask).count_ones()
    }

    /// Play a move given as a single-bit bitmap.
    pub(crate) fn play_bit(&mut self, move_bit: u64) {
        self.current ^= self.mask;
        self.mask |= move_bit;
        self.moves += 1;
    }
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p engine`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add engine/src/board.rs && git commit -m "feat(engine): non-losing move filter and threat heuristic"
```

---

### Task 5: Engine — transposition table

**Files:**
- Modify: `engine/src/solver.rs`

Fixed-size open-addressing-free table: index = key % size, overwrite on collision, full key stored (no false hits). Entry = `(u64, i8)`; value 0 = vacant, stored value = score + `OFFSET`. ~4.2M entries × 16 B ≈ 67 MB.

- [ ] **Step 1: Write failing tests**

In `engine/src/solver.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tt_put_get_roundtrip() {
        let mut tt = TranspositionTable::new();
        tt.put(12345, -3);
        assert_eq!(tt.get(12345), Some(-3));
        assert_eq!(tt.get(99999), None);
    }

    #[test]
    fn tt_collision_overwrites() {
        let mut tt = TranspositionTable::new();
        let k1 = 7u64;
        let k2 = 7u64 + TT_SIZE as u64; // same slot
        tt.put(k1, 5);
        tt.put(k2, -5);
        assert_eq!(tt.get(k2), Some(-5));
        assert_eq!(tt.get(k1), None); // overwritten
    }
}
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p engine tt_`
Expected: compile error.

- [ ] **Step 3: Implement**

`engine/src/solver.rs` (top of file):

```rust
use crate::board::{Position, WIDTH, HEIGHT};

pub(crate) const TT_SIZE: usize = 4_194_301; // prime < 2^22
const OFFSET: i8 = 64; // shift scores so 0 can mean "vacant"

pub(crate) struct TranspositionTable {
    entries: Vec<(u64, i8)>,
}

impl TranspositionTable {
    pub fn new() -> Self {
        TranspositionTable { entries: vec![(0, 0); TT_SIZE] }
    }

    fn index(key: u64) -> usize {
        (key % TT_SIZE as u64) as usize
    }

    pub fn put(&mut self, key: u64, value: i8) {
        self.entries[Self::index(key)] = (key, value + OFFSET);
    }

    pub fn get(&self, key: u64) -> Option<i8> {
        let (k, v) = self.entries[Self::index(key)];
        if v != 0 && k == key {
            Some(v - OFFSET)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.entries.iter_mut().for_each(|e| *e = (0, 0));
    }
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p engine tt_`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add engine/src/solver.rs && git commit -m "feat(engine): transposition table"
```

---

### Task 6: Engine — negamax solver

**Files:**
- Modify: `engine/src/solver.rs`
- Modify: `engine/src/board.rs` (expose two tiny accessors)

Negamax with: non-losing move filter, draw/bound pruning, TT upper bounds, null-window iterative narrowing in `solve()`, move ordering = threat-count heuristic with center-first tie-break.

**Precondition discipline (Pons):** `negamax` is only ever called on positions where the player to move cannot win immediately. `solve()` checks immediate wins first; recursion preserves the invariant because children come from `possible_non_losing_moves` (the child's mover never has an instant win — those moves were filtered as losing for us... the filter removes moves that *give* opponent a win; combined with the explicit pre-check inside the move loop below, the invariant holds).

- [ ] **Step 1: Write failing tests** (append to `mod tests` in `solver.rs`)

```rust
    #[test]
    fn solves_immediate_win() {
        // P1 three-in-column, P1 to move: win on move 7 → score (43-7)/2 = 18
        let p = Position::from_moves("121212").unwrap();
        let mut s = Solver::new();
        assert_eq!(s.solve(&p), 18);
    }

    #[test]
    fn solves_forced_loss() {
        // P2 to move, P1 open three on bottom row (double threat): P1 wins move 9 → P2 score -(43-9)/2 = -17
        let p = Position::from_moves("273747").unwrap();
        let mut s = Solver::new();
        assert_eq!(s.solve(&p), -17);
    }

    #[test]
    fn solves_known_oracle_position() {
        // From connect-four-ai README: score of "76461241141" is -1
        let p = Position::from_moves("76461241141").unwrap();
        let mut s = Solver::new();
        assert_eq!(s.solve(&p), -1);
    }

    #[test]
    #[ignore] // ~minutes without a book; run with: cargo test -p engine --release -- --ignored
    fn solves_empty_board() {
        // Connect-4 theory: first player wins on move 41 → score +1
        let mut s = Solver::new();
        assert_eq!(s.solve(&Position::new()), 1);
    }
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p engine solves`
Expected: compile error — `Solver` undefined.

- [ ] **Step 3: Implement Solver** (append to `solver.rs`)

```rust
/// Static center-first exploration order: 3,2,4,1,5,0,6.
const COLUMN_ORDER: [usize; WIDTH] = [3, 2, 4, 1, 5, 0, 6];

pub struct Solver {
    tt: TranspositionTable,
    pub nodes: u64, // explored node count, handy for benchmarks
}

impl Solver {
    pub fn new() -> Self {
        Solver { tt: TranspositionTable::new(), nodes: 0 }
    }

    /// Exact score of `p` (player-to-move perspective).
    pub fn solve(&mut self, p: &Position) -> i32 {
        let cells = (WIDTH * HEIGHT) as i32;
        if p.moves() as i32 == cells {
            return 0;
        }
        // immediate win?
        if p.can_win_next() {
            return (cells + 1 - p.moves() as i32) / 2;
        }
        // iteratively narrow with null windows
        let mut min = -(cells - p.moves() as i32) / 2;
        let mut max = (cells + 1 - p.moves() as i32) / 2;
        while min < max {
            let mut med = min + (max - min) / 2;
            if med <= 0 && min / 2 < med {
                med = min / 2;
            } else if med >= 0 && max / 2 > med {
                med = max / 2;
            }
            let r = self.negamax(*p, med, med + 1);
            if r <= med {
                max = r;
            } else {
                min = r;
            }
        }
        min
    }

    /// Alpha-beta negamax. Precondition: current player cannot win immediately.
    fn negamax(&mut self, p: Position, mut alpha: i32, mut beta: i32) -> i32 {
        self.nodes += 1;
        let cells = (WIDTH * HEIGHT) as i32;

        let possible = p.possible_non_losing_moves();
        if possible == 0 {
            // every move loses: opponent wins after our next move
            return -(cells - p.moves() as i32) / 2;
        }
        if p.moves() as i32 >= cells - 2 {
            return 0; // draw: at most 2 cells left, no win available
        }

        // lower bound: opponent has no immediate threat, we survive ≥2 more plies
        let min = -(cells - 2 - p.moves() as i32) / 2;
        if alpha < min {
            alpha = min;
            if alpha >= beta {
                return alpha;
            }
        }
        // upper bound: we can't win before move moves+2 (no immediate win by precondition)
        let mut max = (cells - 1 - p.moves() as i32) / 2;
        if let Some(v) = self.tt.get(p.key()) {
            max = v as i32 + MIN_SCORE - 1; // decode stored upper bound
        }
        if beta > max {
            beta = max;
            if alpha >= beta {
                return beta;
            }
        }

        // order moves by threat heuristic, center-first tie-break
        let mut moves: Vec<(u32, u64)> = Vec::with_capacity(WIDTH);
        for (rank, &col) in COLUMN_ORDER.iter().enumerate() {
            let m = possible & Position::column_mask(col);
            if m != 0 {
                // higher threat count first; among equal, earlier in COLUMN_ORDER first
                moves.push((p.move_score(m) * 8 + (WIDTH - rank) as u32, m));
            }
        }
        moves.sort_by(|a, b| b.0.cmp(&a.0));

        for (_, m) in moves {
            let mut child = p;
            child.play_bit(m);
            let score = -self.negamax(child, -beta, -alpha);
            if score >= beta {
                return score;
            }
            if score > alpha {
                alpha = score;
            }
        }

        // store alpha as upper bound for this position
        self.tt.put(p.key(), (alpha - MIN_SCORE + 1) as i8);
        alpha
    }
}

/// Lowest possible score: -(WIDTH*HEIGHT)/2 = -21.
pub(crate) const MIN_SCORE: i32 = -((WIDTH * HEIGHT) as i32) / 2;

impl Default for Solver {
    fn default() -> Self {
        Self::new()
    }
}
```

TT encoding note: `negamax` stores `alpha - MIN_SCORE + 1` (always ≥ 1) and decodes symmetrically on `get`. Keep `TranspositionTable` exactly as Task 5 built it (its internal `OFFSET` stacks harmlessly on top because `put`/`get` are symmetric); the vacant-slot sentinel 0 is never a legal stored value.

- [ ] **Step 4: Run fast tests, verify pass**

Run: `cargo test -p engine solves --release`
Expected: 3 PASS, 1 ignored. (Use `--release` — debug solver is slow.)

- [ ] **Step 5: Run the deep test once**

Run: `cargo test -p engine --release -- --ignored solves_empty_board`
Expected: PASS (may take minutes). If >30 min, abort and continue — the opening book (Task 9) covers deep positions in production; revisit ordering heuristics later.

- [ ] **Step 6: Commit**

```bash
git add engine/src/solver.rs engine/src/board.rs && git commit -m "feat(engine): negamax solver with TT and null-window search"
```

---

### Task 7: Engine — oracle cross-check tests

**Files:**
- Create: `engine/tests/oracle.rs`

Random playouts; compare our score against the independent `connect-four-ai` solver on every prefix position. Deterministic seed → reproducible.

- [ ] **Step 1: Write the test**

`engine/tests/oracle.rs`:

```rust
use engine::{Position, Solver};
use rand::{rngs::StdRng, Rng, SeedableRng};

/// Generate a random legal move string of length `len` (no win occurring mid-game),
/// returning None if the game ended early.
fn random_game(rng: &mut StdRng, len: usize) -> Option<String> {
    let mut p = Position::new();
    let mut s = String::new();
    for _ in 0..len {
        // collect legal, non-immediately-winning columns to keep the game going
        let cols: Vec<usize> = (0..7)
            .filter(|&c| p.can_play(c) && !p.is_winning_move(c))
            .collect();
        if cols.is_empty() {
            return None;
        }
        let col = cols[rng.random_range(0..cols.len())];
        p.play(col);
        s.push(char::from_digit(col as u32 + 1, 10).unwrap());
    }
    Some(s)
}

#[test]
fn matches_oracle_on_random_midgame_positions() {
    let mut rng = StdRng::seed_from_u64(42);
    let mut ours = Solver::new();
    let mut oracle = connect_four_ai::Solver::new();
    let mut checked = 0;
    while checked < 30 {
        // depth 14-26: solvable quickly without a book
        let len = rng.random_range(14..=26);
        let Some(moves) = random_game(&mut rng, len) else { continue };
        let p = Position::from_moves(&moves).unwrap();
        let op = connect_four_ai::Position::from_moves(&moves).unwrap();
        assert_eq!(
            ours.solve(&p),
            oracle.solve(&op),
            "score mismatch on moves {moves}"
        );
        checked += 1;
    }
}
```

Note: if `connect_four_ai`'s API differs in detail (e.g. `from_moves` returns `Result`, or `solve` takes by value), adapt the two oracle lines to the published docs at <https://docs.rs/connect-four-ai/1.0.0> — the assertion stays identical. Same for `rand` 0.10 method names (`random_range` is the 0.9+ name of `gen_range`).

- [ ] **Step 2: Run, verify it actually tests something**

Run: `cargo test -p engine --release --test oracle`
Expected: PASS (30 positions cross-checked). If it FAILS — our solver has a bug; use superpowers:systematic-debugging before touching anything.

- [ ] **Step 3 (optional): gamesolver.org published test sets**

The blog publishes 6×1000 positions with exact scores (files `Test_L3_R1`, `Test_L2_R1`, `Test_L2_R2`, `Test_L1_R1`, `Test_L1_R2`, `Test_L1_R3` at `http://blog.gamesolver.org/data/`; line format: `<move-string> <score>`). If reachable, download into `engine/tests/data/` and add an `#[ignore]`d test that parses each line, runs `Solver::solve`, and asserts the score. Skip this step entirely if the downloads fail — the crate oracle covers correctness.

- [ ] **Step 4: Commit**

```bash
git add engine/tests/ && git commit -m "test(engine): oracle cross-check vs connect-four-ai"
```

---

### Task 8: Engine — analyze + best_move

**Files:**
- Modify: `engine/src/solver.rs`

`analyze` = per-column exact score (None for unplayable). `best_move` = argmax; among ties prefer center order. Exact scores already encode fastest-win/slowest-loss.

- [ ] **Step 1: Write failing tests** (append to `mod tests` in `solver.rs`)

```rust
    #[test]
    fn best_move_takes_immediate_win() {
        let p = Position::from_moves("121212").unwrap();
        let mut s = Solver::new();
        assert_eq!(s.best_move(&p), 0);
    }

    #[test]
    fn best_move_blocks_immediate_threat() {
        let p = Position::from_moves("12121").unwrap(); // P2 must block col 0
        let mut s = Solver::new();
        assert_eq!(s.best_move(&p), 0);
    }

    #[test]
    fn analyze_marks_full_column_none() {
        // "112211221122": cols 0 and 1 each filled bottom→top P1,P2,P1,P2,P1,P2
        // (no run ≥ 3, nobody won, both columns full after 12 moves, P1 to move).
        let p = Position::from_moves("112211221122").unwrap();
        assert!(!p.can_play(0));
        assert!(!p.can_play(1));
        let mut s = Solver::new();
        let a = s.analyze(&p);
        assert!(a[0].is_none());
        assert!(a[1].is_none());
        assert!(a[3].is_some());
    }
```

Column-fill verification: chars of "112211221122" alternate P1,P2 → (P1,c0)(P2,c0)(P1,c1)(P2,c1)(P1,c0)(P2,c0)(P1,c1)(P2,c1)(P1,c0)(P2,c0)(P1,c1)(P2,c1). Each column's stones alternate colors, max vertical run 2.

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p engine best_move --release`
Expected: compile error.

- [ ] **Step 3: Implement** (append to `impl Solver`)

```rust
    /// Exact score for each playable column (None = full).
    pub fn analyze(&mut self, p: &Position) -> [Option<i32>; WIDTH] {
        let cells = (WIDTH * HEIGHT) as i32;
        let mut out = [None; WIDTH];
        for col in 0..WIDTH {
            if !p.can_play(col) {
                continue;
            }
            if p.is_winning_move(col) {
                out[col] = Some((cells + 1 - p.moves() as i32) / 2);
            } else {
                let mut child = *p;
                child.play(col);
                out[col] = Some(-self.solve(&child));
            }
        }
        out
    }

    /// Perfect move: argmax score, center-preferred among ties.
    /// Panics if no legal move exists (caller guarantees game not over).
    pub fn best_move(&mut self, p: &Position) -> usize {
        let scores = self.analyze(p);
        let mut best: Option<(usize, i32)> = None;
        for &col in COLUMN_ORDER.iter() {
            if let Some(s) = scores[col] {
                if best.map_or(true, |(_, bs)| s > bs) {
                    best = Some((col, s));
                }
            }
        }
        best.expect("no legal moves").0
    }
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p engine --release`
Expected: all PASS (ignored test stays ignored).

- [ ] **Step 5: Add oracle check for best-move quality** (append to `engine/tests/oracle.rs`)

```rust
#[test]
fn best_move_never_worsens_outcome() {
    // Perfect play property: score(position) == -score(position after best_move).
    // Positions where best_move wins immediately are skipped (solver's win-score
    // definition makes the relation trivially true there).
    let mut rng = StdRng::seed_from_u64(7);
    let mut s = Solver::new();
    let mut checked = 0;
    while checked < 15 {
        let len = rng.random_range(14..=24);
        let Some(moves) = random_game(&mut rng, len) else { continue };
        let p = Position::from_moves(&moves).unwrap();
        let best = s.best_move(&p);
        if p.is_winning_move(best) {
            checked += 1;
            continue;
        }
        let mut after = p;
        after.play(best);
        assert_eq!(s.solve(&p), -s.solve(&after), "best_move dropped value on {moves}");
        checked += 1;
    }
}
```

- [ ] **Step 6: Run, verify pass**

Run: `cargo test -p engine --release --test oracle`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add engine/src/solver.rs engine/tests/oracle.rs && git commit -m "feat(engine): analyze and perfect best_move"
```

---

### Task 9: Engine — opening book + book_gen

**Files:**
- Modify: `engine/src/book.rs`
- Modify: `engine/src/bin/book_gen.rs`
- Modify: `engine/src/lib.rs` (add `Engine` facade)

Book file format: little-endian, `[u32 magic "C4BK"][u8 depth][u64 count]` then `count` sorted records of `[u64 canonical_key][i8 score]`. Lookup = binary search on canonical key. Scores are from the player-to-move's perspective (works for both mirrored variants since score is mirror-invariant).

- [ ] **Step 1: Write failing tests**

In `engine/src/book.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Position, Solver};

    #[test]
    fn book_roundtrip_and_lookup() {
        let dir = std::env::temp_dir().join("c4book_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("d2.book");

        generate(&path, 2).unwrap(); // tiny book: all positions to depth 2
        let book = Book::load(&path).unwrap();
        assert_eq!(book.depth(), 2);

        // every depth-1 and depth-2 position must be present with the solver's score
        let mut s = Solver::new();
        for c1 in 0..7usize {
            let mut p = Position::new();
            p.play(c1);
            assert_eq!(book.lookup(&p), Some(s.solve(&p) as i8), "depth1 col {c1}");
            for c2 in 0..7usize {
                let mut q = p;
                q.play(c2);
                assert_eq!(book.lookup(&q), Some(s.solve(&q) as i8));
            }
        }
        // depth-3 position absent
        let p3 = Position::from_moves("444").unwrap();
        assert_eq!(book.lookup(&p3), None);
    }

    #[test]
    fn lookup_is_mirror_invariant() {
        let dir = std::env::temp_dir().join("c4book_test2");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("d1.book");
        generate(&path, 1).unwrap();
        let book = Book::load(&path).unwrap();
        let a = Position::from_moves("1").unwrap();
        let b = Position::from_moves("7").unwrap();
        assert_eq!(book.lookup(&a), book.lookup(&b));
        assert!(book.lookup(&a).is_some());
    }
}
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p engine book --release`
Expected: compile error.

- [ ] **Step 3: Implement book** (`engine/src/book.rs`)

```rust
use crate::board::Position;
use crate::solver::Solver;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

const MAGIC: u32 = 0x4334424B; // "C4BK"

pub struct Book {
    depth: u8,
    entries: Vec<(u64, i8)>, // sorted by canonical key
}

impl Book {
    pub fn depth(&self) -> u8 {
        self.depth
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        let mut f = std::fs::File::open(path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, "bad book file");
        if buf.len() < 13 || u32::from_le_bytes(buf[0..4].try_into().unwrap()) != MAGIC {
            return Err(bad());
        }
        let depth = buf[4];
        let count = u64::from_le_bytes(buf[5..13].try_into().unwrap()) as usize;
        if buf.len() != 13 + count * 9 {
            return Err(bad());
        }
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let off = 13 + i * 9;
            let key = u64::from_le_bytes(buf[off..off + 8].try_into().unwrap());
            entries.push((key, buf[off + 8] as i8));
        }
        Ok(Book { depth, entries })
    }

    /// Exact score for `p` if it's in the book (player-to-move perspective).
    pub fn lookup(&self, p: &Position) -> Option<i8> {
        let key = p.canonical_key();
        self.entries
            .binary_search_by_key(&key, |e| e.0)
            .ok()
            .map(|i| self.entries[i].1)
    }
}

/// Generate a book of all positions with 1..=depth stones (game not over).
/// Solves each position; writes the sorted file format described above.
pub fn generate(path: &Path, depth: u8) -> std::io::Result<()> {
    let mut solved: BTreeMap<u64, i8> = BTreeMap::new();
    let mut solver = Solver::new();

    // DFS enumeration with dedup on canonical key.
    fn walk(
        p: &Position,
        remaining: u8,
        solved: &mut BTreeMap<u64, i8>,
        solver: &mut Solver,
    ) {
        if remaining == 0 {
            return;
        }
        for col in 0..crate::board::WIDTH {
            if !p.can_play(col) || p.is_winning_move(col) {
                continue; // finished games don't need book entries
            }
            let mut child = *p;
            child.play(col);
            let key = child.canonical_key();
            if !solved.contains_key(&key) {
                let score = solver.solve(&child) as i8;
                solved.insert(key, score);
                eprintln!("solved {} positions", solved.len());
            }
            walk(&child, remaining - 1, solved, solver);
        }
    }
    walk(&Position::new(), depth, &mut solved, &mut solver);

    let mut out = Vec::with_capacity(13 + solved.len() * 9);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.push(depth);
    out.extend_from_slice(&(solved.len() as u64).to_le_bytes());
    for (k, v) in &solved {
        out.extend_from_slice(&k.to_le_bytes());
        out.push(*v as u8);
    }
    std::fs::File::create(path)?.write_all(&out)
}
```

(Keep the `eprintln!` progress line but rate-limit it during implementation if noisy: `if solved.len() % 1000 == 0`.)

- [ ] **Step 4: Implement book_gen bin** (`engine/src/bin/book_gen.rs`)

```rust
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let depth: u8 = args
        .next()
        .unwrap_or_else(|| "8".into())
        .parse()
        .expect("usage: book_gen [depth] [out_path]");
    let out: PathBuf = args
        .next()
        .unwrap_or_else(|| format!("engine/book_d{depth}.bin"))
        .into();
    println!("generating depth-{depth} book → {}", out.display());
    let start = std::time::Instant::now();
    engine::book::generate(&out, depth).expect("book generation failed");
    println!("done in {:?}", start.elapsed());
}
```

- [ ] **Step 5: Add `Engine` facade** (replace `engine/src/lib.rs`)

```rust
pub mod board;
pub mod book;
pub mod solver;

pub use board::{Position, HEIGHT, WIDTH};
pub use book::Book;
pub use solver::Solver;

use std::path::Path;
use std::sync::Mutex;

/// Thread-safe facade the server uses: book + solver behind one lock.
pub struct Engine {
    book: Option<Book>,
    solver: Mutex<Solver>,
}

impl Engine {
    pub fn new(book_path: Option<&Path>) -> Self {
        let book = book_path.map(|p| Book::load(p).expect("failed to load opening book"));
        Engine { book, solver: Mutex::new(Solver::new()) }
    }

    /// Exact score, book-first.
    pub fn solve(&self, p: &Position) -> i32 {
        if let Some(b) = &self.book {
            if let Some(s) = b.lookup(p) {
                return s as i32;
            }
        }
        self.solver.lock().unwrap().solve(p)
    }

    /// Perfect move, using the book for child scores where possible.
    pub fn best_move(&self, p: &Position) -> usize {
        // immediate win first
        for col in [3, 2, 4, 1, 5, 0, 6] {
            if p.can_play(col) && p.is_winning_move(col) {
                return col;
            }
        }
        let mut best: Option<(usize, i32)> = None;
        for col in [3, 2, 4, 1, 5, 0, 6] {
            if !p.can_play(col) {
                continue;
            }
            let mut child = *p;
            child.play(col);
            let score = -self.solve(&child);
            if best.map_or(true, |(_, bs)| score > bs) {
                best = Some((col, score));
            }
        }
        best.expect("no legal moves").0
    }

    /// Human-readable eval, from P1/P2 absolute perspective.
    /// e.g. "P1 wins in 9 moves", "Draw with perfect play".
    pub fn eval_text(&self, p: &Position) -> String {
        let s = self.solve(p);
        if s == 0 {
            return "Draw with perfect play".into();
        }
        let to_move = if p.moves() % 2 == 0 { 1 } else { 2 };
        let n = p.moves() as i32;
        // plies until forced end (derivation in plan; score = 22 - winner_stone_count)
        let (winner, plies) = if s > 0 {
            (to_move, (if n % 2 == 0 { 43 } else { 44 }) - 2 * s - n)
        } else {
            (3 - to_move, (if n % 2 == 0 { 44 } else { 43 }) + 2 * s - n)
        };
        format!("P{winner} wins in {plies} moves")
    }
}
```

Derivation: score `s = 22 - k` where `k` = winner's stone count at the win (matches the solver's `(cells + 1 - moves) / 2` immediate-win return for both parities). With `n` stones on board, the mover holds `floor(n/2)` stones. Win for mover: mover adds `(22 - s) - floor(n/2)` stones, game ends on their last → `plies = 2·adds − 1`, which simplifies to the parity-split expression above. Loss: opponent (holding `ceil(n/2)`) adds `(22 + s) − ceil(n/2)` stones, ends on theirs → `plies = 2·adds`. Sanity anchors to test in implementation: `"121212"` (n=6, s=18) → "P1 wins in 1 moves"; `"12121"` (n=5, s=−17) → "P1 wins in 4 moves". Add both as unit tests in `lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_text_known_positions() {
        let e = Engine::new(None);
        let p = Position::from_moves("121212").unwrap();
        assert_eq!(e.eval_text(&p), "P1 wins in 1 moves");
        let q = Position::from_moves("12121").unwrap();
        assert_eq!(e.eval_text(&q), "P1 wins in 4 moves");
    }
}
```

- [ ] **Step 6: Run tests, verify pass**

Run: `cargo test -p engine --release`
Expected: all PASS (book tests generate tiny depth-1/2 books in temp dirs).

- [ ] **Step 7: Generate a small production book now, deep book overnight**

Run: `cargo run -p engine --release --bin book_gen -- 6 engine/book.bin`
Expected: completes in reasonable time (likely minutes); prints `done in …`.

Overnight (optional, better bot latency): `cargo run -p engine --release --bin book_gen -- 8 engine/book.bin` — and if that's quick, try 10. Keep whichever depth finished.

- [ ] **Step 8: Commit (including the book)**

```bash
git add engine/src/ engine/book.bin && git commit -m "feat(engine): opening book, generator, Engine facade"
```

---

### Task 10: Server — protocol types

**Files:**
- Create: `server/src/protocol.rs`
- Modify: `server/src/main.rs` (add `mod protocol;`)

- [ ] **Step 1: Write failing round-trip tests**

`server/src/protocol.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_msgs_parse() {
        let m: ClientMsg =
            serde_json::from_str(r#"{"type":"CreateRoom","name":"ty","vs_bot":true}"#).unwrap();
        assert!(matches!(m, ClientMsg::CreateRoom { vs_bot: true, bot_first: false, .. }));

        let m: ClientMsg =
            serde_json::from_str(r#"{"type":"JoinRoom","code":"ABCD","name":"jo"}"#).unwrap();
        assert!(matches!(m, ClientMsg::JoinRoom { token: None, .. }));

        let m: ClientMsg = serde_json::from_str(r#"{"type":"Move","col":3}"#).unwrap();
        assert!(matches!(m, ClientMsg::Move { col: 3 }));
    }

    #[test]
    fn server_msgs_serialize_tagged() {
        let s = serde_json::to_string(&ServerMsg::Error { msg: "nope".into() }).unwrap();
        assert!(s.contains(r#""type":"Error""#));
    }

    #[test]
    fn malformed_json_is_err() {
        assert!(serde_json::from_str::<ClientMsg>(r#"{"type":"Fly"}"#).is_err());
    }
}
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p server`
Expected: compile error.

- [ ] **Step 3: Implement** (`server/src/protocol.rs`)

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ClientMsg {
    CreateRoom {
        name: String,
        vs_bot: bool,
        #[serde(default)]
        bot_first: bool,
    },
    JoinRoom {
        code: String,
        name: String,
        #[serde(default)]
        token: Option<String>,
    },
    Spectate { code: String },
    ListRooms,
    Move { col: usize },
    Rematch,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ServerMsg {
    RoomList { rooms: Vec<RoomInfo> },
    /// seat: 0 = first player (P1 of game 1), 1 = second. 255 = spectator.
    Joined { token: String, seat: u8, code: String, state: GameState },
    State { state: GameState },
    MovePlayed { col: usize, by: u8, eval: Option<String> },
    /// winner: 1|2, 0 = draw. line = winning cells as [col,row].
    GameOver { winner: u8, line: Vec<[usize; 2]> },
    Error { msg: String },
}

#[derive(Serialize, Clone, Debug)]
pub struct RoomInfo {
    pub code: String,
    pub host: String,
    pub vs_bot: bool,
    pub open: bool, // has a free human seat
}

#[derive(Serialize, Clone, Debug)]
pub struct GameState {
    pub board: Vec<Vec<u8>>, // [row][col], row 0 = bottom; 0 empty, 1 P1, 2 P2
    pub turn: u8,            // 1|2
    pub status: String,      // "waiting" | "playing" | "over"
    pub names: [String; 2],
    pub winner: u8,           // 0 = none/draw
    pub line: Vec<[usize; 2]>,
    /// Which seat is P1 this game (flips on rematch) — lets clients map their
    /// fixed seat to a game color.
    pub p1_seat: u8,
}
```

In `server/src/main.rs` add at top: `mod protocol;`

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p server`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/ && git commit -m "feat(server): WS protocol types"
```

---

### Task 11: Server — Game + Room state

**Files:**
- Create: `server/src/room.rs`, `server/src/state.rs`
- Modify: `server/src/main.rs` (add `mod room; mod state;`)

- [ ] **Step 1: Write failing tests**

`server/src/room.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_alternates_and_rejects_illegal() {
        let mut g = Game::new();
        assert!(g.play(3).is_ok());
        assert_eq!(g.turn(), 2);
        let mut h = Game::new();
        for _ in 0..3 {
            h.play(0).unwrap();
            h.play(1).unwrap();
        }
        for _ in 0..3 {
            h.play(1).unwrap();
            h.play(0).unwrap();
        }
        assert!(h.play(0).is_err()); // col 0 now has 6
    }

    #[test]
    fn win_sets_over_winner_and_line() {
        let mut g = Game::new();
        // P1 vertical in col 0: 1,2,1,2,1,2,1 → P1 wins
        for col in [0usize, 1, 0, 1, 0, 1, 0] {
            g.play(col).unwrap();
        }
        assert!(g.over);
        assert_eq!(g.winner, 1);
        assert_eq!(g.line.len(), 4);
        assert!(g.line.contains(&[0, 0]) && g.line.contains(&[0, 3]));
        assert!(g.play(2).is_err()); // game over
    }

    #[test]
    fn snapshot_reflects_board() {
        let mut g = Game::new();
        g.play(3).unwrap();
        let st = g.state(["a".into(), "b".into()], "playing");
        assert_eq!(st.board[0][3], 1);
        assert_eq!(st.turn, 2);
        assert_eq!(st.status, "playing");
    }
}
```

Column-fill verification for `h`: moves alternate players; cols 0,1,0,1,0,1 then 1,0,1,0,1,0 → col 0 bottom-up = P1,P1,P1,P2,P2,P2 and col 1 = P2,P2,P2,P1,P1,P1. Max run 3, no diagonal possible with only two columns, both columns full after 12 moves, nobody won; the 13th call `h.play(0)` must error.

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p server`
Expected: compile error.

- [ ] **Step 3: Implement Game + Room** (`server/src/room.rs`)

```rust
use crate::protocol::{GameState, ServerMsg};
use engine::{Position, HEIGHT, WIDTH};
use std::time::Instant;
use tokio::sync::broadcast;

pub struct Game {
    pub pos: Position,
    pub over: bool,
    pub winner: u8,            // 0 = none/draw
    pub line: Vec<[usize; 2]>, // [col,row] winning cells
}

impl Game {
    pub fn new() -> Self {
        Game { pos: Position::new(), over: false, winner: 0, line: Vec::new() }
    }

    /// 1 or 2 — whose turn.
    pub fn turn(&self) -> u8 {
        if self.pos.moves() % 2 == 0 { 1 } else { 2 }
    }

    pub fn play(&mut self, col: usize) -> Result<(), &'static str> {
        if self.over {
            return Err("game over");
        }
        if col >= WIDTH || !self.pos.can_play(col) {
            return Err("illegal move");
        }
        let mover = self.turn();
        let won = self.pos.is_winning_move(col);
        self.pos.play(col);
        if won {
            self.over = true;
            self.winner = mover;
            self.line = find_line(&self.pos.to_grid(), mover);
        } else if self.pos.moves() as usize == WIDTH * HEIGHT {
            self.over = true; // draw, winner stays 0
        }
        Ok(())
    }

    pub fn state(&self, names: [String; 2], status: &str) -> GameState {
        let g = self.pos.to_grid();
        GameState {
            board: g.iter().map(|row| row.to_vec()).collect(),
            turn: self.turn(),
            status: status.into(),
            names,
            winner: self.winner,
            line: self.line.clone(),
            p1_seat: 0, // Room::snapshot overwrites with the real value
        }
    }
}

/// Scan the grid for `player`'s 4-in-a-row; returns the cells as [col,row].
fn find_line(g: &[[u8; WIDTH]; HEIGHT], player: u8) -> Vec<[usize; 2]> {
    let dirs: [(isize, isize); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
    for row in 0..HEIGHT as isize {
        for col in 0..WIDTH as isize {
            for (dc, dr) in dirs {
                let cells: Vec<[usize; 2]> = (0..4)
                    .map(|i| (col + i * dc, row + i * dr))
                    .filter(|&(c, r)| {
                        (0..WIDTH as isize).contains(&c)
                            && (0..HEIGHT as isize).contains(&r)
                            && g[r as usize][c as usize] == player
                    })
                    .map(|(c, r)| [c as usize, r as usize])
                    .collect();
                if cells.len() == 4 {
                    return cells;
                }
            }
        }
    }
    Vec::new()
}

pub struct PlayerSlot {
    pub name: String,
    pub token: String,
    pub connected: bool,
    /// Bumped when the token reconnects; stale sockets fail this check.
    pub gen: u64,
}

pub struct Room {
    pub code: String,
    pub game: Game,
    pub players: [Option<PlayerSlot>; 2],
    pub bot: Option<u8>, // seat (0|1) the bot occupies
    pub spectators: usize,
    pub tx: broadcast::Sender<ServerMsg>,
    pub last_active: Instant,
    pub rematch_votes: [bool; 2],
    /// Which seat is P1 this game (flips on rematch).
    pub p1_seat: u8,
}

impl Room {
    pub fn new(code: String) -> Self {
        let (tx, _) = broadcast::channel(64);
        Room {
            code,
            game: Game::new(),
            players: [None, None],
            bot: None,
            spectators: 0,
            tx,
            last_active: Instant::now(),
            rematch_votes: [false, false],
            p1_seat: 0,
        }
    }

    pub fn names(&self) -> [String; 2] {
        let n = |s: usize| {
            self.players[s]
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| if self.bot == Some(s as u8) { "Bot".into() } else { "—".into() })
        };
        // names indexed by game color: names[0] = P1's name this game
        if self.p1_seat == 0 { [n(0), n(1)] } else { [n(1), n(0)] }
    }

    pub fn status(&self) -> &'static str {
        if self.game.over {
            "over"
        } else if self.seat_filled(0) && self.seat_filled(1) {
            "playing"
        } else {
            "waiting"
        }
    }

    fn seat_filled(&self, s: usize) -> bool {
        self.players[s].is_some() || self.bot == Some(s as u8)
    }

    /// Game color (1|2) currently assigned to `seat`.
    pub fn color_of_seat(&self, seat: u8) -> u8 {
        if seat == self.p1_seat { 1 } else { 2 }
    }

    /// Seat whose turn it is.
    pub fn seat_to_move(&self) -> u8 {
        if self.game.turn() == 1 { self.p1_seat } else { 1 - self.p1_seat }
    }

    pub fn snapshot(&self) -> GameState {
        let mut st = self.game.state(self.names(), self.status());
        st.p1_seat = self.p1_seat;
        st
    }

    pub fn broadcast(&self, msg: ServerMsg) {
        let _ = self.tx.send(msg); // Err just means no listeners
    }

    /// Reset for rematch: new game, starting seat swaps.
    pub fn rematch(&mut self) {
        self.game = Game::new();
        self.rematch_votes = [false, false];
        self.p1_seat = 1 - self.p1_seat;
    }
}
```

- [ ] **Step 4: Implement AppState** (`server/src/state.rs`)

```rust
use crate::protocol::{RoomInfo, ServerMsg};
use crate::room::Room;
use engine::Engine;
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

pub struct AppState {
    pub rooms: Mutex<HashMap<String, Room>>,
    pub engine: Arc<Engine>,
    /// Lobby feed: RoomList pushed on every room change.
    pub lobby_tx: broadcast::Sender<ServerMsg>,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(engine: Engine) -> SharedState {
        let (lobby_tx, _) = broadcast::channel(64);
        Arc::new(AppState {
            rooms: Mutex::new(HashMap::new()),
            engine: Arc::new(engine),
            lobby_tx,
        })
    }

    pub fn room_list(&self) -> Vec<RoomInfo> {
        self.rooms
            .lock()
            .unwrap()
            .values()
            .map(|r| RoomInfo {
                code: r.code.clone(),
                host: r.players[0]
                    .as_ref()
                    .or(r.players[1].as_ref())
                    .map(|p| p.name.clone())
                    .unwrap_or_default(),
                vs_bot: r.bot.is_some(),
                open: (0..2).any(|s| {
                    r.players[s].is_none() && r.bot != Some(s as u8)
                }),
            })
            .collect()
    }

    pub fn push_lobby_update(&self) {
        let _ = self.lobby_tx.send(ServerMsg::RoomList { rooms: self.room_list() });
    }

    pub fn new_room_code(&self) -> String {
        let rooms = self.rooms.lock().unwrap();
        let mut rng = rand::rng();
        loop {
            let code: String = (0..4)
                .map(|_| rng.random_range(b'A'..=b'Z') as char)
                .collect();
            if !rooms.contains_key(&code) {
                return code;
            }
        }
    }
}
```

In `server/src/main.rs` add: `mod room; mod state;`

(`rand` 0.10 names: `rand::rng()`, `random_range`. If the build errors on those, check `cargo doc -p rand --open` for the current names — 0.8-era names were `thread_rng()`/`gen_range`.)

- [ ] **Step 5: Run tests, verify pass**

Run: `cargo test -p server`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add server/src/ && git commit -m "feat(server): game, room and app state"
```

---

### Task 12: Server — axum app, WS handler, lobby

**Files:**
- Create: `server/src/lib.rs`, `server/src/ws.rs`, `server/src/bot.rs`
- Modify: `server/src/main.rs` (becomes thin binary)
- Modify: `engine/src/lib.rs` (one small method)
- Create: `server/tests/ws.rs` (integration tests + helpers)

Server becomes a lib + thin bin so integration tests can build the router.

**Eval gating rule:** computing `eval_text` solves the position — cheap with a book, minutes without one on early positions. Server only computes eval when `engine.eval_available(moves)`: book loaded OR ≥ 12 stones. Tests run bookless and stay fast; production with book shows eval always.

- [ ] **Step 1: Add `eval_available` to Engine** (append to `impl Engine` in `engine/src/lib.rs`)

```rust
    /// Whether eval display is affordable for a position with `moves` stones.
    pub fn eval_available(&self, moves: u32) -> bool {
        self.book.is_some() || moves >= 12
    }
```

- [ ] **Step 2: Write failing integration tests**

`server/tests/ws.rs`:

```rust
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn spawn_app() -> String {
    let state = server::state::AppState::new(engine::Engine::new(None));
    let app = server::app(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("ws://{addr}/ws")
}

async fn ws(url: &str) -> Ws {
    connect_async(url).await.unwrap().0
}

async fn send(w: &mut Ws, v: Value) {
    w.send(Message::Text(v.to_string().into())).await.unwrap();
}

async fn recv(w: &mut Ws) -> Value {
    loop {
        let m = tokio::time::timeout(std::time::Duration::from_secs(5), w.next())
            .await
            .expect("recv timeout")
            .expect("socket closed")
            .unwrap();
        if let Message::Text(t) = m {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

/// Receive messages until one with `type == ty`, discarding others.
async fn recv_type(w: &mut Ws, ty: &str) -> Value {
    loop {
        let v = recv(w).await;
        if v["type"] == ty {
            return v;
        }
    }
}

#[tokio::test]
async fn create_and_list_rooms() {
    let url = spawn_app().await;
    let mut a = ws(&url).await;
    send(&mut a, json!({"type":"CreateRoom","name":"alice","vs_bot":false})).await;
    let joined = recv_type(&mut a, "Joined").await;
    assert_eq!(joined["seat"], 0);
    let code = joined["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 4);
    assert!(!joined["token"].as_str().unwrap().is_empty());
    assert_eq!(joined["state"]["status"], "waiting");

    let mut b = ws(&url).await;
    send(&mut b, json!({"type":"ListRooms"})).await;
    let list = recv_type(&mut b, "RoomList").await;
    assert_eq!(list["rooms"][0]["code"], code.as_str());
    assert_eq!(list["rooms"][0]["open"], true);
}

#[tokio::test]
async fn join_unknown_room_errors() {
    let url = spawn_app().await;
    let mut a = ws(&url).await;
    send(&mut a, json!({"type":"JoinRoom","code":"ZZZZ","name":"bob"})).await;
    let e = recv_type(&mut a, "Error").await;
    assert!(e["msg"].as_str().unwrap().contains("room"));
}

#[tokio::test]
async fn second_join_starts_game() {
    let url = spawn_app().await;
    let mut a = ws(&url).await;
    send(&mut a, json!({"type":"CreateRoom","name":"alice","vs_bot":false})).await;
    let code = recv_type(&mut a, "Joined").await["code"].as_str().unwrap().to_string();

    let mut b = ws(&url).await;
    send(&mut b, json!({"type":"JoinRoom","code":code,"name":"bob"})).await;
    let joined = recv_type(&mut b, "Joined").await;
    assert_eq!(joined["seat"], 1);
    assert_eq!(joined["state"]["status"], "playing");
    // creator gets the status flip too
    let st = recv_type(&mut a, "State").await;
    assert_eq!(st["state"]["status"], "playing");
}
```

- [ ] **Step 3: Run, verify fail**

Run: `cargo test -p server --test ws`
Expected: compile error — `server::app` missing.

- [ ] **Step 4: Create `server/src/lib.rs`**

```rust
pub mod bot;
pub mod protocol;
pub mod room;
pub mod state;
pub mod ws;

use axum::{routing::any, Router};
use state::SharedState;
use tower_http::services::ServeDir;

pub fn app(state: SharedState) -> Router {
    let static_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/static");
    Router::new()
        .route("/ws", any(ws::ws_handler))
        .fallback_service(ServeDir::new(static_dir))
        .with_state(state)
}
```

Create `server/static/` with an empty `index.html` for now (filled in Task 14):

```html
<!doctype html><title>Connect 4</title>
```

- [ ] **Step 5: Implement WS handler** (`server/src/ws.rs`)

```rust
use crate::protocol::{ClientMsg, ServerMsg};
use crate::room::{PlayerSlot, Room};
use crate::state::SharedState;
use crate::bot;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use std::time::Instant;
use tokio::sync::broadcast;
use uuid::Uuid;

/// What this socket is, once it joined a room.
struct Membership {
    code: String,
    seat: u8, // 0|1 player, 255 spectator
    token: String,
    gen: u64,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<SharedState>) -> Response {
    ws.on_upgrade(|sock| handle_socket(sock, state))
}

async fn handle_socket(mut sock: WebSocket, state: SharedState) {
    let mut room_rx: Option<broadcast::Receiver<ServerMsg>> = None;
    let mut lobby_rx = state.lobby_tx.subscribe();
    let mut me: Option<Membership> = None;

    loop {
        tokio::select! {
            m = sock.recv() => match m {
                Some(Ok(Message::Text(t))) => {
                    let Ok(msg) = serde_json::from_str::<ClientMsg>(&t) else {
                        break; // malformed JSON: close socket
                    };
                    if let Some(reply) = handle_msg(msg, &state, &mut me, &mut room_rx) {
                        let txt = serde_json::to_string(&reply).unwrap();
                        if sock.send(Message::Text(txt.into())).await.is_err() {
                            break;
                        }
                    }
                }
                Some(Ok(_)) => {} // ignore binary/ping/pong (axum answers pings)
                Some(Err(_)) | None => break,
            },
            // room broadcasts (only when joined)
            r = async { room_rx.as_mut().unwrap().recv().await }, if room_rx.is_some() => {
                match r {
                    Ok(msg) => {
                        let txt = serde_json::to_string(&msg).unwrap();
                        if sock.send(Message::Text(txt.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            },
            // lobby feed (only while not in a room)
            l = lobby_rx.recv(), if me.is_none() => {
                if let Ok(msg) = l {
                    let txt = serde_json::to_string(&msg).unwrap();
                    if sock.send(Message::Text(txt.into())).await.is_err() {
                        break;
                    }
                }
            },
        }
    }

    // disconnect: mark slot, fix spectator count
    if let Some(m) = &me {
        let mut rooms = state.rooms.lock().unwrap();
        if let Some(room) = rooms.get_mut(&m.code) {
            if m.seat == 255 {
                room.spectators = room.spectators.saturating_sub(1);
            } else if let Some(slot) = room.players[m.seat as usize].as_mut() {
                if slot.gen == m.gen {
                    slot.connected = false;
                }
            }
        }
    }
}

/// Handle one client message. Returns the direct reply (if any); room-wide
/// effects go through broadcasts inside.
fn handle_msg(
    msg: ClientMsg,
    state: &SharedState,
    me: &mut Option<Membership>,
    room_rx: &mut Option<broadcast::Receiver<ServerMsg>>,
) -> Option<ServerMsg> {
    match msg {
        ClientMsg::ListRooms => Some(ServerMsg::RoomList { rooms: state.room_list() }),

        ClientMsg::CreateRoom { name, vs_bot, bot_first } => {
            let code = state.new_room_code();
            let mut room = Room::new(code.clone());
            let token = Uuid::new_v4().to_string();
            let human_seat = if vs_bot && bot_first { 1 } else { 0 };
            if vs_bot {
                room.bot = Some(if bot_first { 0 } else { 1 });
            }
            room.players[human_seat] =
                Some(PlayerSlot { name, token: token.clone(), connected: true, gen: 0 });
            *room_rx = Some(room.tx.subscribe());
            let snapshot = room.snapshot();
            state.rooms.lock().unwrap().insert(code.clone(), room);
            *me = Some(Membership { code: code.clone(), seat: human_seat as u8, token: token.clone(), gen: 0 });
            state.push_lobby_update();
            if vs_bot && bot_first {
                bot::spawn_bot_move(state.clone(), code.clone());
            }
            Some(ServerMsg::Joined { token, seat: human_seat as u8, code, state: snapshot })
        }

        ClientMsg::JoinRoom { code, name, token } => {
            let mut rooms = state.rooms.lock().unwrap();
            let Some(room) = rooms.get_mut(&code) else {
                return Some(ServerMsg::Error { msg: "no such room".into() });
            };
            // reconnect path: token matches an existing seat
            if let Some(tok) = token {
                let found = room.players.iter().position(|p| {
                    p.as_ref().is_some_and(|p| p.token == tok)
                });
                if let Some(seat) = found {
                    let slot = room.players[seat].as_mut().unwrap();
                    slot.connected = true;
                    slot.gen += 1; // stale sockets with old gen can no longer act
                    let gen = slot.gen;
                    *room_rx = Some(room.tx.subscribe());
                    let snapshot = room.snapshot();
                    *me = Some(Membership { code: code.clone(), seat: seat as u8, token: tok.clone(), gen });
                    return Some(ServerMsg::Joined { token: tok, seat: seat as u8, code, state: snapshot });
                }
            }
            // fresh seat
            let free = (0..2).find(|&s| room.players[s].is_none() && room.bot != Some(s as u8));
            let Some(seat) = free else {
                return Some(ServerMsg::Error { msg: "room full".into() });
            };
            let token = Uuid::new_v4().to_string();
            room.players[seat] =
                Some(PlayerSlot { name, token: token.clone(), connected: true, gen: 0 });
            room.last_active = Instant::now();
            *room_rx = Some(room.tx.subscribe());
            let snapshot = room.snapshot();
            room.broadcast(ServerMsg::State { state: snapshot.clone() });
            *me = Some(Membership { code: code.clone(), seat: seat as u8, token: token.clone(), gen: 0 });
            drop(rooms);
            state.push_lobby_update();
            Some(ServerMsg::Joined { token, seat: seat as u8, code, state: snapshot })
        }

        ClientMsg::Spectate { code } => {
            let mut rooms = state.rooms.lock().unwrap();
            let Some(room) = rooms.get_mut(&code) else {
                return Some(ServerMsg::Error { msg: "no such room".into() });
            };
            room.spectators += 1;
            *room_rx = Some(room.tx.subscribe());
            let snapshot = room.snapshot();
            *me = Some(Membership { code: code.clone(), seat: 255, token: String::new(), gen: 0 });
            Some(ServerMsg::Joined { token: String::new(), seat: 255, code, state: snapshot })
        }

        ClientMsg::Move { col } => handle_move(col, state, me.as_ref()?),

        ClientMsg::Rematch => handle_rematch(state, me.as_ref()?),
    }
}

fn handle_move(col: usize, state: &SharedState, me: &Membership) -> Option<ServerMsg> {
    if me.seat > 1 {
        return Some(ServerMsg::Error { msg: "spectators can't move".into() });
    }
    let mut rooms = state.rooms.lock().unwrap();
    let Some(room) = rooms.get_mut(&me.code) else {
        return Some(ServerMsg::Error { msg: "room gone".into() });
    };
    // stale-socket guard (older tab after a reconnect)
    let live = room.players[me.seat as usize].as_ref().is_some_and(|p| p.gen == me.gen);
    if !live {
        return Some(ServerMsg::Error { msg: "stale session".into() });
    }
    if room.status() != "playing" {
        return Some(ServerMsg::Error { msg: "game not in progress".into() });
    }
    if room.seat_to_move() != me.seat {
        return Some(ServerMsg::Error { msg: "not your turn".into() });
    }
    let by = room.game.turn();
    if let Err(e) = room.game.play(col) {
        return Some(ServerMsg::Error { msg: e.into() });
    }
    room.last_active = Instant::now();
    room.broadcast(ServerMsg::State { state: room.snapshot() });
    if room.game.over {
        room.broadcast(ServerMsg::GameOver {
            winner: room.game.winner,
            line: room.game.line.clone(),
        });
        drop(rooms);
        state.push_lobby_update();
        return None;
    }
    let pos = room.game.pos;
    let bot_turn = room.bot == Some(room.seat_to_move());
    let code = me.code.clone();
    drop(rooms);
    spawn_eval(state.clone(), code.clone(), col, by, pos);
    if bot_turn {
        bot::spawn_bot_move(state.clone(), code);
    }
    None
}

fn handle_rematch(state: &SharedState, me: &Membership) -> Option<ServerMsg> {
    if me.seat > 1 {
        return Some(ServerMsg::Error { msg: "spectators can't vote".into() });
    }
    let mut rooms = state.rooms.lock().unwrap();
    let Some(room) = rooms.get_mut(&me.code) else {
        return Some(ServerMsg::Error { msg: "room gone".into() });
    };
    if !room.game.over {
        return Some(ServerMsg::Error { msg: "game not over".into() });
    }
    room.rematch_votes[me.seat as usize] = true;
    let all_voted = (0..2).all(|s| {
        room.bot == Some(s as u8) || room.rematch_votes[s] || room.players[s].is_none()
    });
    if all_voted {
        room.rematch();
        room.last_active = Instant::now();
        room.broadcast(ServerMsg::State { state: room.snapshot() });
        if room.bot == Some(room.seat_to_move()) {
            let code = me.code.clone();
            drop(rooms);
            bot::spawn_bot_move(state.clone(), code);
        }
    }
    None
}

/// Compute and broadcast the eval for the position after a human move.
fn spawn_eval(state: SharedState, code: String, col: usize, by: u8, pos: engine::Position) {
    if !state.engine.eval_available(pos.moves()) {
        return;
    }
    tokio::spawn(async move {
        let engine = state.engine.clone();
        let Ok(eval) = tokio::task::spawn_blocking(move || engine.eval_text(&pos)).await else {
            return; // eval is decoration; a panic here shouldn't kill anything
        };
        let rooms = state.rooms.lock().unwrap();
        if let Some(room) = rooms.get(&code) {
            room.broadcast(ServerMsg::MovePlayed { col, by, eval: Some(eval) });
        }
    });
}
```

- [ ] **Step 6: Implement bot task** (`server/src/bot.rs`)

```rust
use crate::protocol::ServerMsg;
use crate::state::SharedState;
use std::time::Instant;

/// Compute and apply the bot's move for `code`, if it's the bot's turn.
pub fn spawn_bot_move(state: SharedState, code: String) {
    tokio::spawn(async move {
        // snapshot the position the bot must answer
        let pos = {
            let rooms = state.rooms.lock().unwrap();
            let Some(room) = rooms.get(&code) else { return };
            if room.game.over || room.bot != Some(room.seat_to_move()) {
                return;
            }
            room.game.pos
        };

        let engine = state.engine.clone();
        let result = tokio::task::spawn_blocking(move || {
            let col = engine.best_move(&pos);
            let eval = if pos.is_winning_move(col) {
                None // game ends; GameOver says it all
            } else {
                let mut after = pos;
                after.play(col);
                engine.eval_available(after.moves()).then(|| engine.eval_text(&after))
            };
            (col, eval)
        })
        .await;

        let mut rooms = state.rooms.lock().unwrap();
        let Some(room) = rooms.get_mut(&code) else { return };

        let Ok((col, eval)) = result else {
            // engine panicked (JoinError): forfeit gracefully, human wins
            room.game.over = true;
            room.game.winner = 3 - room.game.turn();
            room.broadcast(ServerMsg::Error { msg: "bot crashed — game forfeited".into() });
            room.broadcast(ServerMsg::GameOver { winner: room.game.winner, line: vec![] });
            return;
        };

        if room.game.pos != pos {
            return; // room changed under us (rematch/reset) — drop the stale move
        }
        let by = room.game.turn();
        if room.game.play(col).is_err() {
            return;
        }
        room.last_active = Instant::now();
        room.broadcast(ServerMsg::State { state: room.snapshot() });
        room.broadcast(ServerMsg::MovePlayed { col, by, eval });
        if room.game.over {
            room.broadcast(ServerMsg::GameOver {
                winner: room.game.winner,
                line: room.game.line.clone(),
            });
        }
    });
}
```

- [ ] **Step 7: Thin `server/src/main.rs`**

```rust
use std::time::Duration;

#[tokio::main]
async fn main() {
    let book_path = std::path::Path::new("engine/book.bin");
    let engine = engine::Engine::new(book_path.exists().then_some(book_path));
    if book_path.exists() {
        println!("opening book loaded from {}", book_path.display());
    } else {
        eprintln!("WARNING: no engine/book.bin — bot/eval will be slow on early moves");
    }
    let state = server::state::AppState::new(engine);

    // reap rooms idle > 1h
    let reaper = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            let removed = {
                let mut rooms = reaper.rooms.lock().unwrap();
                let before = rooms.len();
                rooms.retain(|_, r| r.last_active.elapsed() < Duration::from_secs(3600));
                before - rooms.len()
            };
            if removed > 0 {
                reaper.push_lobby_update();
            }
        }
    });

    let app = server::app(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Connect-4 server: http://<your-LAN-IP>:3000");
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 8: Run tests, verify pass**

Run: `cargo test -p server`
Expected: protocol/room unit tests + 3 ws integration tests PASS.

- [ ] **Step 9: Commit**

```bash
git add server/ engine/src/lib.rs && git commit -m "feat(server): axum app, WS handler, lobby, bot task"
```

---

### Task 13: Server — gameplay integration tests (win, illegal, reconnect, rematch, spectate, bot)

**Files:**
- Modify: `server/tests/ws.rs`

The handler logic already exists (Task 12); these tests exercise it end-to-end and will flush out bugs. Fix any failures in `ws.rs`/`room.rs` — the tests are the contract.

- [ ] **Step 1: Append gameplay tests** (to `server/tests/ws.rs`)

```rust
/// A creates, B joins; returns (a, b, code, token_a).
async fn start_game(url: &str) -> (Ws, Ws, String, String) {
    let mut a = ws(url).await;
    send(&mut a, json!({"type":"CreateRoom","name":"alice","vs_bot":false})).await;
    let j = recv_type(&mut a, "Joined").await;
    let code = j["code"].as_str().unwrap().to_string();
    let token_a = j["token"].as_str().unwrap().to_string();
    let mut b = ws(url).await;
    send(&mut b, json!({"type":"JoinRoom","code":code,"name":"bob"})).await;
    recv_type(&mut b, "Joined").await;
    recv_type(&mut a, "State").await; // status flip
    (a, b, code, token_a)
}

#[tokio::test]
async fn full_game_vertical_win() {
    let url = spawn_app().await;
    let (mut a, mut b, _, _) = start_game(&url).await;
    // A: col 0 ×4, B: col 1 ×3 → A wins
    for (i, col) in [0, 1, 0, 1, 0, 1, 0].iter().enumerate() {
        let w: &mut Ws = if i % 2 == 0 { &mut a } else { &mut b };
        send(w, json!({"type":"Move","col":col})).await;
        recv_type(&mut a, "State").await;
        recv_type(&mut b, "State").await;
    }
    let over = recv_type(&mut a, "GameOver").await;
    assert_eq!(over["winner"], 1);
    assert_eq!(over["line"].as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn illegal_moves_rejected() {
    let url = spawn_app().await;
    let (mut a, mut b, _, _) = start_game(&url).await;
    // not B's turn
    send(&mut b, json!({"type":"Move","col":3})).await;
    let e = recv_type(&mut b, "Error").await;
    assert_eq!(e["msg"], "not your turn");
    // bad column
    send(&mut a, json!({"type":"Move","col":9})).await;
    let e = recv_type(&mut a, "Error").await;
    assert_eq!(e["msg"], "illegal move");
    // board unchanged: A can still play col 3 fine
    send(&mut a, json!({"type":"Move","col":3})).await;
    let st = recv_type(&mut a, "State").await;
    assert_eq!(st["state"]["board"][0][3], 1);
}

#[tokio::test]
async fn reconnect_reclaims_seat() {
    let url = spawn_app().await;
    let (mut a, mut b, code, token_a) = start_game(&url).await;
    send(&mut a, json!({"type":"Move","col":3})).await;
    recv_type(&mut b, "State").await;
    drop(a); // alice's wifi dies

    let mut a2 = ws(&url).await;
    send(&mut a2, json!({"type":"JoinRoom","code":code,"name":"alice","token":token_a})).await;
    let j = recv_type(&mut a2, "Joined").await;
    assert_eq!(j["seat"], 0);
    assert_eq!(j["state"]["board"][0][3], 1); // board snapshot intact

    // B moves, then reconnected A moves — game continues normally
    send(&mut b, json!({"type":"Move","col":3})).await;
    recv_type(&mut a2, "State").await;
    send(&mut a2, json!({"type":"Move","col":2})).await;
    let st = recv_type(&mut b, "State").await;
    assert_eq!(st["state"]["board"][0][2], 1);
}

#[tokio::test]
async fn rematch_swaps_starting_player() {
    let url = spawn_app().await;
    let (mut a, mut b, _, _) = start_game(&url).await;
    for (i, col) in [0, 1, 0, 1, 0, 1, 0].iter().enumerate() {
        let w: &mut Ws = if i % 2 == 0 { &mut a } else { &mut b };
        send(w, json!({"type":"Move","col":col})).await;
        recv_type(&mut a, "State").await;
        recv_type(&mut b, "State").await;
    }
    recv_type(&mut a, "GameOver").await;
    send(&mut a, json!({"type":"Rematch"})).await;
    send(&mut b, json!({"type":"Rematch"})).await;
    let st = recv_type(&mut b, "State").await;
    assert_eq!(st["state"]["status"], "playing");
    assert_eq!(st["state"]["names"][0], "bob"); // seats swapped: bob is P1 now
    // bob (P1 of game 2) moves first
    send(&mut b, json!({"type":"Move","col":3})).await;
    let st = recv_type(&mut a, "State").await;
    assert_eq!(st["state"]["board"][0][3], 1);
}

#[tokio::test]
async fn spectator_sees_moves() {
    let url = spawn_app().await;
    let (mut a, mut b, code, _) = start_game(&url).await;
    let mut c = ws(&url).await;
    send(&mut c, json!({"type":"Spectate","code":code})).await;
    let j = recv_type(&mut c, "Joined").await;
    assert_eq!(j["seat"], 255);
    send(&mut a, json!({"type":"Move","col":4})).await;
    let st = recv_type(&mut c, "State").await;
    assert_eq!(st["state"]["board"][0][4], 1);
    // spectator can't move
    send(&mut c, json!({"type":"Move","col":0})).await;
    let e = recv_type(&mut c, "Error").await;
    assert_eq!(e["msg"], "spectators can't move");
    let _ = recv_type(&mut b, "State").await;
}

/// Needs engine/book.bin (Task 9 step 7). Run: cargo test -p server --release -- --ignored
#[tokio::test]
#[ignore]
async fn bot_replies_and_never_hangs() {
    let book = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../engine/book.bin"));
    assert!(book.exists(), "generate engine/book.bin first (Task 9)");
    let state = server::state::AppState::new(engine::Engine::new(Some(book)));
    let app = server::app(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let url = format!("ws://{addr}/ws");

    let mut a = ws(&url).await;
    send(&mut a, json!({"type":"CreateRoom","name":"human","vs_bot":true})).await;
    recv_type(&mut a, "Joined").await;
    // play a naive line; bot must respond to every move until the game ends
    'outer: for col in [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4] {
        send(&mut a, json!({"type":"Move","col":col})).await;
        loop {
            let v = recv(&mut a).await;
            match v["type"].as_str().unwrap() {
                "GameOver" => {
                    assert_eq!(v["winner"], 2, "perfect bot (P2) cannot lose to this line");
                    return;
                }
                // our move was illegal (column filled, or game already over and
                // the queued GameOver arrives next loop) — try the next column
                "Error" => continue 'outer,
                // bot (P2) replied — proceed to our next move.
                // (If the bot's move ended the game, the GameOver is already
                // queued and the Error path above routes us back here to it.)
                "MovePlayed" if v["by"] == 2 => continue 'outer,
                _ => {} // States, our own MovePlayed eval — keep consuming
            }
        }
    }
    panic!("game never ended");
}
```

Note on the ignored bot test: it is intentionally loose (naive client, may hit Error on filled columns) — its job is "bot always replies, game terminates, bot doesn't lose". Perfect-play guarantees are proven at the engine layer (oracle + property tests + Task 15).

- [ ] **Step 2: Run, fix, verify pass**

Run: `cargo test -p server`
Expected: all non-ignored tests PASS. Any failure is a real handler bug — debug with superpowers:systematic-debugging, not by weakening the test.

- [ ] **Step 3: Commit**

```bash
git add server/tests/ws.rs && git commit -m "test(server): gameplay, reconnect, rematch, spectate integration tests"
```

---

### Task 14: Frontend

**Files:**
- Modify: `server/static/index.html`
- Create: `server/static/style.css`, `server/static/app.js`

No build step, no framework. One page, two views (`#lobby`, `#game`). Client is fully server-driven: every render comes from a `State` snapshot.

**Requires a protocol addition:** the client must know which game color (1|2) its seat has after rematch swaps — add `p1_seat` to `GameState` (see Step 1). This field was already included in Tasks 10/11 code if you're executing in order; if not, retrofit it now.

- [ ] **Step 1: Verify `p1_seat` is in GameState**

`server/src/protocol.rs` `GameState` must contain `pub p1_seat: u8`, and `Room::snapshot()` must set it (`st.p1_seat = self.p1_seat`). If missing, add both, run `cargo test -p server` (still green), commit as `fix(server): expose p1_seat in snapshots`.

- [ ] **Step 2: Write `server/static/index.html`**

```html
<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Connect 4</title>
<link rel="stylesheet" href="style.css">
</head>
<body>
<div id="lobby" class="view">
  <h1>Connect 4</h1>
  <label>Your name <input id="name" maxlength="16" placeholder="anon"></label>
  <div class="actions">
    <button id="create-human">Create game vs human</button>
    <button id="create-bot">Create game vs bot</button>
    <label><input type="checkbox" id="bot-first"> bot moves first</label>
  </div>
  <div class="joinrow">
    <input id="join-code" placeholder="CODE" maxlength="4">
    <button id="join-btn">Join</button>
    <button id="spectate-btn">Spectate</button>
  </div>
  <h2>Open games</h2>
  <ul id="rooms"></ul>
</div>

<div id="game" class="view hidden">
  <div id="topbar">
    <span id="room-code"></span>
    <span id="turn-label"></span>
    <label><input type="checkbox" id="show-eval" checked> eval</label>
  </div>
  <div id="board"></div>
  <div id="eval-line"></div>
  <div id="banner" class="hidden">
    <span id="banner-text"></span>
    <button id="rematch">Rematch</button>
    <button id="leave">Back to lobby</button>
  </div>
</div>
<div id="toast" class="hidden"></div>
<script src="app.js"></script>
</body>
</html>
```

- [ ] **Step 3: Write `server/static/style.css`**

```css
:root { --p1: #e74c3c; --p2: #f1c40f; --bg: #1e2a38; --cell: #2c3e50; }
* { box-sizing: border-box; font-family: system-ui, sans-serif; }
body { margin: 0; background: var(--bg); color: #ecf0f1; display: flex; justify-content: center; }
.view { max-width: 480px; width: 100%; padding: 16px; }
.hidden { display: none !important; }
h1 { text-align: center; }
input, button { font-size: 1rem; padding: 8px 12px; border-radius: 6px; border: none; }
button { background: #3498db; color: white; cursor: pointer; }
button:hover { background: #2980b9; }
.actions, .joinrow { display: flex; gap: 8px; margin: 12px 0; flex-wrap: wrap; align-items: center; }
#join-code { width: 6ch; text-transform: uppercase; }
#rooms { list-style: none; padding: 0; }
#rooms li { background: var(--cell); margin: 6px 0; padding: 10px; border-radius: 6px;
            display: flex; justify-content: space-between; cursor: pointer; }
#rooms li:hover { outline: 2px solid #3498db; }

#topbar { display: flex; justify-content: space-between; align-items: center; margin: 8px 0; }
#board { display: grid; grid-template-columns: repeat(7, 1fr); gap: 6px;
         background: #34495e; padding: 10px; border-radius: 10px; }
.cell { aspect-ratio: 1; border-radius: 50%; background: var(--cell); }
.cell.p1 { background: var(--p1); }
.cell.p2 { background: var(--p2); }
.cell.win { box-shadow: 0 0 0 4px #2ecc71 inset; }
.cell.playable { cursor: pointer; }
.col-hover .cell.playable { filter: brightness(1.4); }
#eval-line { min-height: 1.4em; margin-top: 8px; color: #95a5a6; font-style: italic; }
#banner { margin-top: 12px; display: flex; gap: 8px; align-items: center; }
#toast { position: fixed; bottom: 16px; left: 50%; transform: translateX(-50%);
         background: #c0392b; padding: 10px 16px; border-radius: 6px; }
```

- [ ] **Step 4: Write `server/static/app.js`**

```javascript
"use strict";
const $ = (id) => document.getElementById(id);
let ws = null;
let my = JSON.parse(localStorage.getItem("c4") || "null"); // {code, token, seat}
let spectator = false;
let lastState = null;
let lastLine = [];

const nameVal = () => $("name").value.trim() || "anon";
const send = (o) => ws && ws.readyState === 1 && ws.send(JSON.stringify(o));

function connect() {
  ws = new WebSocket(`ws://${location.host}/ws`);
  ws.onopen = () => {
    if (my && my.token) {
      send({ type: "JoinRoom", code: my.code, name: nameVal(), token: my.token });
    } else {
      send({ type: "ListRooms" });
    }
  };
  ws.onmessage = (e) => handle(JSON.parse(e.data));
  ws.onclose = () => setTimeout(connect, 1000); // auto-reconnect
}

function handle(m) {
  switch (m.type) {
    case "RoomList":
      renderRooms(m.rooms);
      break;
    case "Joined":
      spectator = m.seat === 255;
      if (!spectator) {
        my = { code: m.code, token: m.token, seat: m.seat };
        localStorage.setItem("c4", JSON.stringify(my));
      }
      $("room-code").textContent = `room ${m.code}`;
      $("lobby").classList.add("hidden");
      $("game").classList.remove("hidden");
      lastLine = [];
      render(m.state);
      break;
    case "State":
      if (m.state.status === "playing" && lastState && lastState.status === "over") {
        lastLine = []; // rematch started
        $("banner").classList.add("hidden");
        $("eval-line").textContent = "";
      }
      render(m.state);
      break;
    case "MovePlayed":
      if (m.eval && $("show-eval").checked) $("eval-line").textContent = m.eval;
      break;
    case "GameOver":
      lastLine = m.line;
      render(lastState);
      $("banner-text").textContent =
        m.winner === 0 ? "Draw!" : `${lastState.names[m.winner - 1]} wins!`;
      $("rematch").classList.toggle("hidden", spectator);
      $("banner").classList.remove("hidden");
      break;
    case "Error":
      toast(m.msg);
      if (m.msg === "no such room") leave(); // saved room got reaped
      break;
  }
}

function myColor(state) {
  if (spectator || !my) return 0;
  return my.seat === state.p1_seat ? 1 : 2;
}

function render(state) {
  if (!state) return;
  lastState = state;
  const board = $("board");
  board.innerHTML = "";
  const mine = myColor(state);
  const myTurn = state.status === "playing" && state.turn === mine;
  // display rows top (5) to bottom (0)
  for (let row = 5; row >= 0; row--) {
    for (let col = 0; col < 7; col++) {
      const cell = document.createElement("div");
      cell.className = "cell";
      const v = state.board[row][col];
      if (v) cell.classList.add(v === 1 ? "p1" : "p2");
      if (lastLine.some(([c, r]) => c === col && r === row)) cell.classList.add("win");
      if (myTurn && state.board[5][col] === 0) {
        cell.classList.add("playable");
        cell.dataset.col = col;
        cell.onclick = () => send({ type: "Move", col });
      }
      board.appendChild(cell);
    }
  }
  const names = state.names;
  $("turn-label").textContent =
    state.status === "waiting" ? "waiting for opponent…"
    : state.status === "over" ? "game over"
    : myTurn ? "your turn"
    : `${names[state.turn - 1]}'s turn`;
}

function renderRooms(rooms) {
  const ul = $("rooms");
  ul.innerHTML = "";
  for (const r of rooms) {
    const li = document.createElement("li");
    li.innerHTML = `<span>${r.code} — ${r.host}${r.vs_bot ? " 🤖" : ""}</span>
                    <span>${r.open ? "join" : "spectate"}</span>`;
    li.onclick = () =>
      send(r.open ? { type: "JoinRoom", code: r.code, name: nameVal() }
                  : { type: "Spectate", code: r.code });
    ul.appendChild(li);
  }
}

function leave() {
  localStorage.removeItem("c4");
  my = null;
  location.reload();
}

let toastTimer = null;
function toast(msg) {
  $("toast").textContent = msg;
  $("toast").classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => $("toast").classList.add("hidden"), 3000);
}

$("create-human").onclick = () =>
  send({ type: "CreateRoom", name: nameVal(), vs_bot: false });
$("create-bot").onclick = () =>
  send({ type: "CreateRoom", name: nameVal(), vs_bot: true, bot_first: $("bot-first").checked });
$("join-btn").onclick = () =>
  send({ type: "JoinRoom", code: $("join-code").value.toUpperCase(), name: nameVal() });
$("spectate-btn").onclick = () =>
  send({ type: "Spectate", code: $("join-code").value.toUpperCase() });
$("rematch").onclick = () => send({ type: "Rematch" });
$("leave").onclick = leave;

connect();
```

- [ ] **Step 5: Manual verification checklist**

Run: `cargo run -p server --release` (from `final_project/`)

Open two browser windows at `http://localhost:3000`:
1. Window 1: set name, "Create game vs human" → board shows "waiting for opponent…"
2. Window 2: room appears in lobby list; click it → both boards go live
3. Play moves alternately — turn label and stones update live in both
4. Make a winning line — green-ringed cells + banner with winner name
5. Both click Rematch — new game, other player starts
6. Refresh window 1 mid-game — board restores, can keep playing (reconnect)
7. Window 3 (or incognito): Spectate via code — sees live moves, gets Error on click
8. "Create game vs bot" — bot answers each move (instantly with book)
9. Eval line appears under board after moves (with book), toggle hides it

If a LAN device is handy: `http://<host-LAN-IP>:3000` from a phone — same checks.

- [ ] **Step 6: Commit**

```bash
git add server/static/ server/src/ && git commit -m "feat(frontend): lobby and game UI"
```

---

### Task 15: Perfect-play e2e + README

**Files:**
- Create: `engine/tests/perfect_play.rs`
- Create: `README.md` (in `final_project/`)

- [ ] **Step 1: Write the bot-vs-bot theory test**

`engine/tests/perfect_play.rs`:

```rust
use engine::{Engine, Position};
use std::path::Path;

/// Connect-4 theory: with perfect play on both sides, the first player
/// wins by move 41. The strongest single smoke test of the whole engine.
/// Needs engine/book.bin. Run: cargo test -p engine --release -- --ignored bot_vs_bot
#[test]
#[ignore]
fn bot_vs_bot_first_player_wins_move_41() {
    let book = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/book.bin"));
    assert!(
        book.exists(),
        "generate first: cargo run -p engine --release --bin book_gen -- 8 engine/book.bin"
    );
    let e = Engine::new(Some(book));
    let mut p = Position::new();
    loop {
        assert!(p.moves() < 42, "perfect game cannot be a draw");
        let col = e.best_move(&p);
        if p.is_winning_move(col) {
            assert_eq!(p.moves() % 2, 0, "P1 must win a perfect game");
            assert_eq!(p.moves() + 1, 41, "perfect game ends exactly on move 41");
            return;
        }
        p.play(col);
    }
}
```

- [ ] **Step 2: Run it once**

Run: `cargo test -p engine --release -- --ignored bot_vs_bot`
Expected: PASS. Slow mid-game moves are normal with a shallow book; with a depth-8+ book it's minutes total. A failure here is a serious engine bug — systematic-debugging time.

- [ ] **Step 3: Write `README.md`**

```markdown
# Connect-4 LAN Server

Locally hosted Connect-4: players on the same network play in the browser,
or face a perfect-play bot backed by a from-scratch Rust solver.

## Quick start

​```bash
# one-time: generate the opening book (deeper = snappier bot; 8 is a good target)
cargo run -p engine --release --bin book_gen -- 8 engine/book.bin

# run the server (from this directory)
cargo run -p server --release
​```

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

​```bash
cargo test --workspace --release            # fast suite (incl. oracle cross-checks)
cargo test --workspace --release -- --ignored   # deep: empty-board solve, bot-vs-bot e2e
​```
```

(Remove the zero-width escapes around the inner code fences — they're only here to nest fences in this plan.)

- [ ] **Step 4: Final verification**

Run: `cargo test --workspace --release`
Expected: everything green.

Run: `cargo clippy --workspace -- -D warnings`
Expected: clean (fix what it flags).

- [ ] **Step 5: Commit**

```bash
git add engine/tests/perfect_play.rs README.md && git commit -m "test: perfect-play e2e; docs: README"
```

---

## Execution Notes

- **Order matters:** engine (2-9) before server (10-13) before frontend (14). Task 9's book generation can run in the background while server tasks proceed.
- **All `cargo test` on engine should use `--release`** — the solver in debug mode is 10-50× slower.
- **Crate-API drift:** exact method names for `rand` 0.10 / `connect-four-ai` 1.0 / tokio-tungstenite 0.29 may differ slightly from the snippets; check docs.rs when a snippet doesn't compile rather than pinning older versions.
- After the final task, use superpowers:finishing-a-development-branch.





