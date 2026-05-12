// Toads and Frogs — Minimax & Minimax with α/β Pruning
// T = Toad (moves right), F = Frog (moves left), _ = empty
// Toads = maximiser (+1), Frogs = minimiser (-1). No moves = loss.

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    Toad,
    Frog,
    Empty,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Player {
    Toads,
    Frogs,
}

impl Player {
    fn opponent(self) -> Player {
        match self {
            Player::Toads => Player::Frogs,
            Player::Frogs => Player::Toads,
        }
    }
}

pub type Board = Vec<Cell>;

// ── Move generation ──────────────────────────────────────────────────────────

fn legal_moves(board: &Board, player: Player) -> Vec<(usize, usize)> {
    let n = board.len();
    let mut moves = Vec::new();

    match player {
        Player::Toads => {
            for i in 0..n {
                if board[i] == Cell::Toad {
                    // Step right
                    if i + 1 < n && board[i + 1] == Cell::Empty {
                        moves.push((i, i + 1));
                    }
                    // Jump right over a Frog
                    if i + 2 < n && board[i + 1] == Cell::Frog && board[i + 2] == Cell::Empty {
                        moves.push((i, i + 2));
                    }
                }
            }
        }
        Player::Frogs => {
            for i in (0..n).rev() {
                if board[i] == Cell::Frog {
                    // Step left
                    if i >= 1 && board[i - 1] == Cell::Empty {
                        moves.push((i, i - 1));
                    }
                    // Jump left over a Toad
                    if i >= 2 && board[i - 1] == Cell::Toad && board[i - 2] == Cell::Empty {
                        moves.push((i, i - 2));
                    }
                }
            }
        }
    }

    moves
}

fn apply_move(board: &Board, from: usize, to: usize) -> Board {
    let mut b = board.clone();
    b[to] = b[from];
    b[from] = Cell::Empty;
    b
}

// ── Problem 1 — Plain Minimax ─────────────────────────────────────────────────

#[derive(Debug)]
pub struct SearchResult {
    pub score: i32,                    // +1 Toads win, -1 Frogs win
    pub best_move: Option<(usize, usize)>,
    pub nodes_visited: u64,
}

fn minimax_inner(board: &Board, player: Player) -> (i32, u64) {
    let moves = legal_moves(board, player);

    // Terminal: current player has no moves → they lose
    if moves.is_empty() {
        return (
            match player {
                Player::Toads => -1, // Toads cannot move → Frogs win
                Player::Frogs => 1,  // Frogs cannot move → Toads win
            },
            1,
        );
    }

    let mut total_nodes: u64 = 1;

    let best_score = match player {
        Player::Toads => {
            let mut best = i32::MIN;
            for (from, to) in &moves {
                let child = apply_move(board, *from, *to);
                let (score, nodes) = minimax_inner(&child, player.opponent());
                total_nodes += nodes;
                if score > best {
                    best = score;
                }
            }
            best
        }
        Player::Frogs => {
            let mut best = i32::MAX;
            for (from, to) in &moves {
                let child = apply_move(board, *from, *to);
                let (score, nodes) = minimax_inner(&child, player.opponent());
                total_nodes += nodes;
                if score < best {
                    best = score;
                }
            }
            best
        }
    };

    (best_score, total_nodes)
}

pub fn minimax(board: &Board, player: Player) -> SearchResult {
    let moves = legal_moves(board, player);

    if moves.is_empty() {
        let score = match player {
            Player::Toads => -1,
            Player::Frogs => 1,
        };
        return SearchResult {
            score,
            best_move: None,
            nodes_visited: 1,
        };
    }

    let mut best_score = match player {
        Player::Toads => i32::MIN,
        Player::Frogs => i32::MAX,
    };
    let mut best_move = moves[0];
    let mut total_nodes: u64 = 1;

    for (from, to) in &moves {
        let child = apply_move(board, *from, *to);
        let (score, nodes) = minimax_inner(&child, player.opponent());
        total_nodes += nodes;

        let better = match player {
            Player::Toads => score > best_score,
            Player::Frogs => score < best_score,
        };
        if better {
            best_score = score;
            best_move = (*from, *to);
        }
    }

    SearchResult {
        score: best_score,
        best_move: Some(best_move),
        nodes_visited: total_nodes,
    }
}

// ── Problem 2 — Minimax with α/β Pruning ─────────────────────────────────────

fn alphabeta_inner(board: &Board, player: Player, mut alpha: i32, mut beta: i32) -> (i32, u64) {
    let moves = legal_moves(board, player);

    if moves.is_empty() {
        return (
            match player {
                Player::Toads => -1,
                Player::Frogs => 1,
            },
            1,
        );
    }

    let mut total_nodes: u64 = 1;

    match player {
        Player::Toads => {
            let mut value = i32::MIN;
            for (from, to) in &moves {
                let child = apply_move(board, *from, *to);
                let (score, nodes) = alphabeta_inner(&child, player.opponent(), alpha, beta);
                total_nodes += nodes;
                if score > value {
                    value = score;
                }
                if value > alpha {
                    alpha = value;
                }
                if value >= beta {
                    break;
                }
            }
            (value, total_nodes)
        }
        Player::Frogs => {
            let mut value = i32::MAX;
            for (from, to) in &moves {
                let child = apply_move(board, *from, *to);
                let (score, nodes) = alphabeta_inner(&child, player.opponent(), alpha, beta);
                total_nodes += nodes;
                if score < value {
                    value = score;
                }
                if value < beta {
                    beta = value;
                }
                if value <= alpha {
                    break;
                }
            }
            (value, total_nodes)
        }
    }
}

pub fn alphabeta(board: &Board, player: Player) -> SearchResult {
    let moves = legal_moves(board, player);

    if moves.is_empty() {
        let score = match player {
            Player::Toads => -1,
            Player::Frogs => 1,
        };
        return SearchResult {
            score,
            best_move: None,
            nodes_visited: 1,
        };
    }

    let mut best_score = match player {
        Player::Toads => i32::MIN,
        Player::Frogs => i32::MAX,
    };
    let mut best_move = moves[0];
    let mut total_nodes: u64 = 1;
    let mut alpha = i32::MIN;
    let mut beta = i32::MAX;

    for (from, to) in &moves {
        let child = apply_move(board, *from, *to);
        let (score, nodes) = alphabeta_inner(&child, player.opponent(), alpha, beta);
        total_nodes += nodes;

        let better = match player {
            Player::Toads => score > best_score,
            Player::Frogs => score < best_score,
        };
        if better {
            best_score = score;
            best_move = (*from, *to);
        }

        match player {
            Player::Toads => { if best_score > alpha { alpha = best_score; } }
            Player::Frogs => { if best_score < beta  { beta  = best_score; } }
        }
    }

    SearchResult {
        score: best_score,
        best_move: Some(best_move),
        nodes_visited: total_nodes,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub fn parse_board(s: &str) -> Board {
    s.chars()
        .map(|c| match c {
            'T' => Cell::Toad,
            'F' => Cell::Frog,
            _ => Cell::Empty,
        })
        .collect()
}

pub fn board_str(board: &Board) -> String {
    board
        .iter()
        .map(|c| match c {
            Cell::Toad => 'T',
            Cell::Frog => 'F',
            Cell::Empty => '_',
        })
        .collect()
}

fn outcome(score: i32) -> &'static str {
    if score == 1 { "Toads WIN" } else { "Frogs WIN" }
}

fn run_both(label: &str, board_str_in: &str, player: Player) {
    let board = parse_board(board_str_in);
    let mm = minimax(&board, player);
    let ab = alphabeta(&board, player);

    println!("┌─ {label}");
    println!("│  Board : {}", board_str_in);
    println!("│  Mover : {:?}", player);
    println!(
        "│  Minimax   → {}  best move: {:?}  nodes: {}",
        outcome(mm.score),
        mm.best_move,
        mm.nodes_visited
    );
    println!(
        "│  AlphaBeta → {}  best move: {:?}  nodes: {}",
        outcome(ab.score),
        ab.best_move,
        ab.nodes_visited
    );
    assert_eq!(mm.score, ab.score, "Score mismatch in {label}!");
    let reduction = if mm.nodes_visited > 0 {
        100.0 - (ab.nodes_visited as f64 / mm.nodes_visited as f64) * 100.0
    } else {
        0.0
    };
    println!("│  Pruning saved {reduction:.1}% of nodes");
    println!("└─");
    println!();
}

// ── Problem 3 — Test Cases ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Asserts both searches agree on score and ab visits no more nodes than mm.
    fn check(board_str_in: &str, player: Player, expected_score: i32) {
        let board = parse_board(board_str_in);
        let mm = minimax(&board, player);
        let ab = alphabeta(&board, player);
        assert_eq!(mm.score, expected_score, "Minimax score wrong for '{board_str_in}'");
        assert_eq!(ab.score, expected_score, "AlphaBeta score wrong for '{board_str_in}'");
        assert!(
            ab.nodes_visited <= mm.nodes_visited,
            "AlphaBeta visited MORE nodes than Minimax for '{board_str_in}': ab={} mm={}",
            ab.nodes_visited, mm.nodes_visited
        );
    }

    #[test] fn simple_toad_win()       { check("T_F",     Player::Toads, 1);  }
    #[test] fn two_each_toads_move()   { check("TT_FF",   Player::Toads, 1);  }
    #[test] fn toads_already_stuck()   { check("FTT",     Player::Toads, -1); }
    #[test] fn frogs_already_stuck()   { check("FFT",     Player::Frogs, 1);  }
    #[test] fn frogs_to_move_wins()    { check("T_F",     Player::Frogs, -1); }
    #[test] fn three_each_toads_move() { check("TTT_FFF", Player::Toads, 1);  }
    #[test] fn no_frogs()              { check("T__",     Player::Toads, 1);  }

    // Large board with real branching — shows significant pruning benefit.
    #[test]
    fn large_pruning_comparison() {
        let board = parse_board("TF_TF_TF_");
        let mm = minimax(&board, Player::Toads);
        let ab = alphabeta(&board, Player::Toads);
        assert_eq!(mm.score, ab.score, "Scores must match");
        println!(
            "[large_comparison] Minimax nodes: {}  AlphaBeta nodes: {}  reduction: {:.1}%",
            mm.nodes_visited, ab.nodes_visited,
            100.0 - (ab.nodes_visited as f64 / mm.nodes_visited as f64) * 100.0
        );
        assert!(
            ab.nodes_visited < mm.nodes_visited,
            "Expected ab nodes < mm nodes: mm={} ab={}",
            mm.nodes_visited, ab.nodes_visited,
        );
    }
}

// ── main — demo all test cases with printed output ────────────────────────────

fn main() {
    println!("═══════════════════════════════════════════════════════════");
    println!("  Toads and Frogs — Minimax vs α/β Pruning");
    println!("  T = Toad (moves right)   F = Frog (moves left)   _ = empty");
    println!("═══════════════════════════════════════════════════════════\n");

    // ── 5+ shared test cases ──────────────────────────────────────────────────
    run_both(
        "Case 1: T_F — Toads move (simplest game)",
        "T_F",
        Player::Toads,
    );
    run_both("Case 2: T_F — Frogs move", "T_F", Player::Frogs);
    run_both(
        "Case 3: TT_FF — Toads move (classic 2-each)",
        "TT_FF",
        Player::Toads,
    );
    run_both(
        "Case 4: FTT — Toads stuck (immediate loss)",
        "FTT",
        Player::Toads,
    );
    run_both(
        "Case 5: FFT — Frogs stuck (immediate loss)",
        "FFT",
        Player::Frogs,
    );
    run_both(
        "Case 6: TTT_FFF — 3-each, Toads move",
        "TTT_FFF",
        Player::Toads,
    );
    run_both(
        "Case 7: T__ — No frogs, Toads move (Frogs lose instantly)",
        "T__",
        Player::Toads,
    );

    // ── Cases showing pruning benefit (boards with real branching) ───────────
    println!("═══════════════════════════════════════════════════════════");
    println!("  PRUNING COMPARISON CASES (multiple legal moves per position)");
    println!("═══════════════════════════════════════════════════════════\n");
    run_both(
        "Case 8: T_TF_F — Toad can step or jump-over-Frog at root",
        "T_TF_F",
        Player::Toads,
    );
    run_both(
        "Case 9: T_FT_F — two independent Toad/Frog sub-games",
        "T_FT_F",
        Player::Toads,
    );
    run_both(
        "Case 10: TF_TF_TF_ — larger branching game",
        "TF_TF_TF_",
        Player::Toads,
    );
}
