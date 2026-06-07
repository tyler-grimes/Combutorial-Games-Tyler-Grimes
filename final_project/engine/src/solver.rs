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

/// Static center-first exploration order: 3,2,4,1,5,0,6.
const COLUMN_ORDER: [usize; WIDTH] = [3, 2, 4, 1, 5, 0, 6];

/// Lowest possible score: -(WIDTH*HEIGHT)/2 = -21.
pub(crate) const MIN_SCORE: i32 = -((WIDTH * HEIGHT) as i32) / 2;

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

        // lower bound: opponent has no immediate threat, we survive >= 2 more plies
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

impl Default for Solver {
    fn default() -> Self {
        Self::new()
    }
}

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
        let p = Position::from_moves("37475").unwrap();
        let mut s = Solver::new();
        assert_eq!(s.solve(&p), -18);
    }

    #[test]
    fn solves_known_oracle_position() {
        // From connect-four-ai: score of "76461241141" is -1
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
}
