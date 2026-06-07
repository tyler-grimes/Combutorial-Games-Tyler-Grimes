use engine::board::Position;
use engine::solver::Solver;
use rand::{rngs::StdRng, RngExt, SeedableRng};

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
    // Use the oracle solver without the opening book to avoid any book-vs-search discrepancy
    let mut oracle = connect_four_ai::Solver::empty();
    let mut checked = 0;
    while checked < 30 {
        // depth 14-26: solvable quickly without a book
        let len = rng.random_range(14..=26usize);
        let Some(moves) = random_game(&mut rng, len) else { continue };
        let p = Position::from_moves(&moves).unwrap();
        // The oracle's from_moves returns a Result; it also rejects winning moves mid-sequence,
        // but our random_game never generates those, so unwrap is safe here.
        let op = connect_four_ai::Position::from_moves(&moves).unwrap();
        let our_score = ours.solve(&p);
        // Oracle returns i8; cast to i32 for comparison with our i32 score.
        let oracle_score = oracle.solve(&op) as i32;
        assert_eq!(
            our_score,
            oracle_score,
            "score mismatch on moves {moves}: ours={our_score}, oracle={oracle_score}"
        );
        checked += 1;
    }
}
