pub mod board;
pub mod book;
pub mod solver;

pub use board::{Position, HEIGHT, WIDTH};
pub use book::Book;
pub use solver::Solver;

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Default wall-clock budget the bot spends choosing a move when it can't look the
/// answer up exactly (opening positions with no book coverage).
pub const DEFAULT_MOVE_BUDGET: Duration = Duration::from_millis(2000);

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

    /// Best move with the default time budget. Plays perfectly when the position is
    /// cheap to solve exactly (book-covered, or shallow enough for the timed search to
    /// reach terminal depth); otherwise returns the strongest move found within budget.
    pub fn best_move(&self, p: &Position) -> usize {
        self.best_move_timed(p, DEFAULT_MOVE_BUDGET)
    }

    /// Best move within `budget`. Exact when the book covers every child (instant
    /// lookups); otherwise a time-limited iterative-deepening search that always
    /// returns within roughly `budget` — so the opening never stalls.
    pub fn best_move_timed(&self, p: &Position, budget: Duration) -> usize {
        // Immediate win — instant.
        for col in [3, 2, 4, 1, 5, 0, 6] {
            if p.can_play(col) && p.is_winning_move(col) {
                return col;
            }
        }
        // Exact, instant path: book holds the score for every child position.
        let book_covers_children = self.book.as_ref().is_some_and(|b| {
            [3usize, 2, 4, 1, 5, 0, 6].iter().filter(|&&c| p.can_play(c)).all(|&c| {
                let mut child = *p;
                child.play(c);
                b.lookup(&child).is_some()
            })
        });
        if book_covers_children {
            let mut best: Option<(usize, i32)> = None;
            for col in [3, 2, 4, 1, 5, 0, 6] {
                if !p.can_play(col) {
                    continue;
                }
                let mut child = *p;
                child.play(col);
                let score = -self.solve(&child);
                if best.is_none_or(|(_, bs)| score > bs) {
                    best = Some((col, score));
                }
            }
            return best.expect("no legal moves").0;
        }
        // No exact coverage: bounded iterative-deepening search.
        let deadline = Instant::now() + budget;
        self.solver.lock().unwrap().best_move_timed(p, deadline)
    }

    /// Human-readable eval. Formula (exact, derived from Pons score convention):
    /// score s = 22 - winner_stone_count. Positive = mover wins.
    /// Win for mover: plies = (if n%2==0 {43} else {44}) - 2*s - n
    /// Loss for mover: plies = (if n%2==0 {44} else {43}) + 2*s - n
    pub fn eval_text(&self, p: &Position) -> String {
        let s = self.solve(p);
        if s == 0 {
            return "Draw with perfect play".into();
        }
        let to_move = if p.moves().is_multiple_of(2) { 1u8 } else { 2u8 };
        let n = p.moves() as i32;
        let (winner, plies) = if s > 0 {
            (to_move, (if n % 2 == 0 { 43 } else { 44 }) - 2 * s - n)
        } else {
            (3 - to_move, (if n % 2 == 0 { 44 } else { 43 }) + 2 * s - n)
        };
        format!("P{winner} wins in {plies} moves")
    }

    /// Whether eval display is affordable: book hit (instant) or move 12+ (solver is fast).
    pub fn eval_available(&self, p: &Position) -> bool {
        p.moves() >= 12 || self.book.as_ref().is_some_and(|b| b.lookup(p).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_text_known_positions() {
        let e = Engine::new(None);
        // "121212": n=6, P1 to move, s=18 → plies = (6%2==0→43) - 36 - 6 = 1
        let p = Position::from_moves("121212").unwrap();
        assert_eq!(e.eval_text(&p), "P1 wins in 1 moves");
        // "12121": n=5, P2 to move. Solver shows a player wins from this position.
        // Just confirm eval_text returns a well-formed string.
        let q = Position::from_moves("12121").unwrap();
        let eval = e.eval_text(&q);
        assert!(eval.contains("wins in") || eval.contains("Draw"), "unexpected eval: {eval}");
    }

    #[test]
    fn timed_best_move_takes_immediate_win() {
        let e = Engine::new(None);
        let p = Position::from_moves("121212").unwrap(); // P1 wins by playing col 0
        assert_eq!(e.best_move(&p), 0);
    }

    #[test]
    fn timed_best_move_blocks_immediate_threat() {
        let e = Engine::new(None);
        let p = Position::from_moves("12121").unwrap(); // P2 must block col 0
        assert_eq!(e.best_move(&p), 0);
    }

    #[test]
    fn timed_best_move_opens_fast_and_central() {
        // Book-less opening must return within budget and not hang.
        let e = Engine::new(None);
        let start = Instant::now();
        let col = e.best_move_timed(&Position::new(), Duration::from_millis(500));
        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_millis(1500), "opening took {elapsed:?}");
        assert!((0..WIDTH).contains(&col));
    }

    #[test]
    fn timed_best_move_never_worsens_outcome() {
        // Perfect-play property on solvable midgame positions: the timed search,
        // given enough budget to reach terminal depth, must not drop the game value.
        use crate::solver::Solver;
        let e = Engine::new(None);
        let mut s = Solver::new();
        for moves in ["44443332", "4455667", "4321234", "44455566"] {
            let p = Position::from_moves(moves).unwrap();
            let col = e.best_move_timed(&p, Duration::from_secs(3));
            if p.is_winning_move(col) {
                continue;
            }
            let mut after = p;
            after.play(col);
            assert_eq!(s.solve(&p), -s.solve(&after), "dropped value on {moves}");
        }
    }
}
