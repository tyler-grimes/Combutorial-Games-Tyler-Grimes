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
