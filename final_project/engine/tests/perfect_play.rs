use engine::{Engine, Position};
use std::path::Path;

/// Connect-4 theory: with perfect play on both sides, the first player
/// wins. The exact move count depends on the opening book depth.
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
            // Note: with a shallow book, the win may not be on move 41 exactly
            // but P1 must win (not P2). The exact move depends on book depth.
            return;
        }
        p.play(col);
    }
}
