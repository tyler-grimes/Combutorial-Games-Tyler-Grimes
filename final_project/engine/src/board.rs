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
            if !(1..=WIDTH).contains(&col) || !p.can_play(col - 1) {
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
        let (p1, p2) = if self.moves.is_multiple_of(2) {
            (self.current, self.current ^ self.mask)
        } else {
            (self.current ^ self.mask, self.current)
        };
        for (row, grow) in g.iter_mut().enumerate() {
            for (col, cell) in grow.iter_mut().enumerate() {
                let bit = 1u64 << (col * H1 + row);
                if p1 & bit != 0 {
                    *cell = 1;
                } else if p2 & bit != 0 {
                    *cell = 2;
                }
            }
        }
        g
    }

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
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}

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
        let p = Position::from_moves("122373347474").unwrap();
        assert_eq!(p.moves(), 12);
        assert!(p.is_winning_move(3));
        assert!(!p.is_winning_move(0));
    }

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
        // P1 with open three 3,4,5 on bottom row (1-indexed): threats at cols 2 and 6.
        // P1 plays 3,4,5 (moves 1,3,5); P2 stacks col 7 (moves 2,4). P2 to move.
        let p = Position::from_moves("37475").unwrap();
        assert_eq!(p.possible_non_losing_moves(), 0);
    }

    #[test]
    fn move_score_counts_created_threats() {
        let p = Position::new();
        // any first move creates no immediate threats
        let m = Position::column_mask(3) & p.possible();
        assert_eq!(p.move_score(m), 0);
    }
}
