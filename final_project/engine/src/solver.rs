use crate::board::{Position, WIDTH, HEIGHT};
use std::time::Instant;

/// Mate score for the timed searcher. Far above any heuristic leaf value so a proven
/// win/loss always dominates. Encodes distance via move count: sooner mates score higher.
const MATE: i32 = 1_000_000;
/// Any |score| at or above this is a proven mate (heuristic values stay well below).
const MATE_THRESHOLD: i32 = MATE - 1_000;
/// Search window infinity.
const INF: i32 = MATE * 2;

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

    /// Exact score for each playable column (None = full).
    pub fn analyze(&mut self, p: &Position) -> [Option<i32>; WIDTH] {
        let cells = (WIDTH * HEIGHT) as i32;
        let mut out = [None; WIDTH];
        for (col, slot) in out.iter_mut().enumerate() {
            if !p.can_play(col) {
                continue;
            }
            if p.is_winning_move(col) {
                *slot = Some((cells + 1 - p.moves() as i32) / 2);
            } else {
                let mut child = *p;
                child.play(col);
                *slot = Some(-self.solve(&child));
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
                if best.is_none_or(|(_, bs)| s > bs) {
                    best = Some((col, s));
                }
            }
        }
        best.expect("no legal moves").0
    }

    /// Strongest move within a wall-clock budget, via iterative-deepening alpha-beta.
    /// Returns exact play when the search reaches terminal depth (shallow trees, late
    /// game); otherwise a heuristic best move from the deepest fully-completed depth.
    /// Always returns within roughly `deadline`, so the opening never stalls.
    pub fn best_move_timed(&mut self, p: &Position, deadline: Instant) -> usize {
        // Immediate win — instant, no search needed.
        for &col in COLUMN_ORDER.iter() {
            if p.can_play(col) && p.is_winning_move(col) {
                return col;
            }
        }
        // Candidates: non-losing moves if any exist, else any playable (already lost,
        // but must still return a legal move), in center-first order.
        let non_losing = p.possible_non_losing_moves();
        let mut candidates: Vec<usize> = COLUMN_ORDER
            .iter()
            .copied()
            .filter(|&c| p.can_play(c) && (non_losing == 0 || non_losing & Position::column_mask(c) != 0))
            .collect();
        if candidates.is_empty() {
            candidates = (0..WIDTH).filter(|&c| p.can_play(c)).collect();
        }
        let mut best = candidates[0];
        // Track per-candidate scores to order the next iteration best-first (PV move).
        let mut scored: Vec<(usize, i32)> = candidates.iter().map(|&c| (c, 0)).collect();
        let max_depth = (WIDTH * HEIGHT) as u32 - p.moves();

        for depth in 1..=max_depth {
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            let mut alpha = -INF;
            let mut iter_best: Option<(usize, i32)> = None;
            let mut completed = true;
            for entry in scored.iter_mut() {
                let mut child = *p;
                child.play(entry.0);
                match self.search(&child, depth - 1, -INF, -alpha, deadline) {
                    Some(s) => {
                        let score = -s;
                        entry.1 = score;
                        if iter_best.is_none_or(|(_, bs)| score > bs) {
                            iter_best = Some((entry.0, score));
                        }
                        if score > alpha {
                            alpha = score;
                        }
                    }
                    None => {
                        completed = false;
                        break;
                    }
                }
            }
            if let Some((col, score)) = iter_best {
                // Commit the deepest fully-searched result; on a partial depth keep the
                // previous depth's move (a half-finished depth can be misleading).
                if completed {
                    best = col;
                    // Proven win/loss within the horizon — no deeper search can improve it.
                    if score.abs() >= MATE_THRESHOLD {
                        break;
                    }
                }
            }
            if !completed {
                break;
            }
        }
        best
    }

    /// Depth-limited negamax with heuristic leaf eval. `None` => aborted past deadline.
    /// Score is from the perspective of the player to move in `p`.
    fn search(&self, p: &Position, depth: u32, mut alpha: i32, beta: i32, deadline: Instant) -> Option<i32> {
        // Win available now: take it. Sooner mates score higher (fewer moves played).
        if p.can_win_next() {
            return Some(MATE - p.moves() as i32);
        }
        let possible = p.possible_non_losing_moves();
        if possible == 0 {
            // Board full => draw; otherwise every reply loses (opponent wins next ply).
            if p.possible() == 0 {
                return Some(0);
            }
            return Some(-(MATE - p.moves() as i32 - 1));
        }
        if depth == 0 {
            return Some(Self::heuristic(p));
        }
        // Check the clock here (interior nodes), not at the leaves, to bound overhead.
        if Instant::now() >= deadline {
            return None;
        }
        // Order moves: more created threats first, center-first tie-break.
        let mut moves: Vec<(u32, u64)> = Vec::with_capacity(WIDTH);
        for (rank, &col) in COLUMN_ORDER.iter().enumerate() {
            let m = possible & Position::column_mask(col);
            if m != 0 {
                moves.push((p.move_score(m) * 8 + (WIDTH - rank) as u32, m));
            }
        }
        moves.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, m) in moves {
            let mut child = *p;
            child.play_bit(m);
            let score = -self.search(&child, depth - 1, -beta, -alpha, deadline)?;
            if score >= beta {
                return Some(score);
            }
            if score > alpha {
                alpha = score;
            }
        }
        Some(alpha)
    }

    /// Leaf heuristic (player-to-move perspective): threat differential + center control.
    fn heuristic(p: &Position) -> i32 {
        let my_threats = p.winning_position().count_ones() as i32;
        let opp_threats = p.opponent_winning_position().count_ones() as i32;
        3 * (my_threats - opp_threats) + p.center_bonus()
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
}
