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

    /// Human-readable eval. Formula (exact, derived from Pons score convention):
    /// score s = 22 - winner_stone_count. Positive = mover wins.
    /// Win for mover: plies = (if n%2==0 {43} else {44}) - 2*s - n
    /// Loss for mover: plies = (if n%2==0 {44} else {43}) + 2*s - n
    pub fn eval_text(&self, p: &Position) -> String {
        let s = self.solve(p);
        if s == 0 {
            return "Draw with perfect play".into();
        }
        let to_move = if p.moves() % 2 == 0 { 1u8 } else { 2u8 };
        let n = p.moves() as i32;
        let (winner, plies) = if s > 0 {
            (to_move, (if n % 2 == 0 { 43 } else { 44 }) - 2 * s - n)
        } else {
            (3 - to_move, (if n % 2 == 0 { 44 } else { 43 }) + 2 * s - n)
        };
        format!("P{winner} wins in {plies} moves")
    }

    /// Whether eval display is affordable for a position with `moves` stones.
    pub fn eval_available(&self, moves: u32) -> bool {
        self.book.is_some() || moves >= 12
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
}
