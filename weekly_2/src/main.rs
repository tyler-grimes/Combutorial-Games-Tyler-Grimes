// Toads and Frogs — Problems 1–5
//
// Board encoding:  T = Toad (moves right), F = Frog (moves left), _ = empty
// A position is represented as a Vec<Cell>.
// Moves:
//   T can step right into an empty cell, or hop right over one F into empty
//   F can step left  into an empty cell, or hop left  over one T into empty

use std::collections::VecDeque;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Cell {
    Toad,  // T
    Frog,  // F
    Empty, // _
}

use Cell::*;

// ─────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────

/// Collect all legal next positions reachable from `pos` in one move.
fn successors(pos: &[Cell]) -> Vec<Vec<Cell>> {
    let n = pos.len();
    let mut result = Vec::new();

    for i in 0..n {
        match pos[i] {
            Toad => {
                // step right into empty
                if i + 1 < n && pos[i + 1] == Empty {
                    let mut next = pos.to_vec();
                    next[i] = Empty;
                    next[i + 1] = Toad;
                    result.push(next);
                }
                // hop right over a Frog into empty
                if i + 2 < n && pos[i + 1] == Frog && pos[i + 2] == Empty {
                    let mut next = pos.to_vec();
                    next[i] = Empty;
                    next[i + 2] = Toad;
                    result.push(next);
                }
            }
            Frog => {
                // step left into empty
                if i >= 1 && pos[i - 1] == Empty {
                    let mut next = pos.to_vec();
                    next[i] = Empty;
                    next[i - 1] = Frog;
                    result.push(next);
                }
                // hop left over a Toad into empty
                if i >= 2 && pos[i - 1] == Toad && pos[i - 2] == Empty {
                    let mut next = pos.to_vec();
                    next[i] = Empty;
                    next[i - 2] = Frog;
                    result.push(next);
                }
            }
            Empty => {}
        }
    }
    result
}

/// Pretty-print a position.
fn display(pos: &[Cell]) -> String {
    pos.iter()
        .map(|c| match c {
            Toad => 'T',
            Frog => 'F',
            Empty => '_',
        })
        .collect()
}

// ─────────────────────────────────────────────────────────
// Problem 1 — all positions of size n
// ─────────────────────────────────────────────────────────

/// Returns every possible board of length `n`  (3^n total).
pub fn all_positions(n: usize) -> Vec<Vec<Cell>> {
    let total = 3usize.pow(n as u32);
    let mut result = Vec::with_capacity(total);
    for mask in 0..total {
        let mut pos = Vec::with_capacity(n);
        let mut m = mask;
        for _ in 0..n {
            pos.push(match m % 3 {
                0 => Empty,
                1 => Toad,
                _ => Frog,
            });
            m /= 3;
        }
        result.push(pos);
    }
    result
}

// ─────────────────────────────────────────────────────────
// Problem 2 — positions with exactly t Toads and f Frogs
// ─────────────────────────────────────────────────────────

/// Returns every board of length `n` with exactly `t` Toads and `f` Frogs.
/// The remaining `n - t - f` cells are Empty.
pub fn positions_with_counts(n: usize, t: usize, f: usize) -> Vec<Vec<Cell>> {
    if t + f > n {
        return vec![];
    }
    let empties = n - t - f;
    let mut result = Vec::new();
    // Iterative DFS with state (partial_pos, toads_remaining, frogs_remaining, empties_remaining)
    let mut stack: Vec<(Vec<Cell>, usize, usize, usize)> = vec![(vec![], t, f, empties)];
    while let Some((pos, tl, fl, el)) = stack.pop() {
        if pos.len() == n {
            result.push(pos);
            continue;
        }
        // push choices — order reversed so pop gives natural order
        if el > 0 {
            let mut p = pos.clone();
            p.push(Empty);
            stack.push((p, tl, fl, el - 1));
        }
        if fl > 0 {
            let mut p = pos.clone();
            p.push(Frog);
            stack.push((p, tl, fl - 1, el));
        }
        if tl > 0 {
            let mut p = pos.clone();
            p.push(Toad);
            stack.push((p, tl - 1, fl, el));
        }
    }
    result
}

// ─────────────────────────────────────────────────────────
// Problem 3 — timing tables
// ─────────────────────────────────────────────────────────

fn table_problem1(max_n: usize) {
    println!("\n=== Problem 1: All positions of size n ===");
    println!("{:<6} {:>15} {:>14}", "n", "positions", "time (s)");
    println!("{}", "-".repeat(38));
    for n in 3..=max_n {
        let start = Instant::now();
        let positions = all_positions(n);
        let elapsed = start.elapsed().as_secs_f64();
        println!("{:<6} {:>15} {:>14.6}", n, positions.len(), elapsed);
        if elapsed > 1800.0 {
            println!("  (stopping: exceeded 30 min)");
            break;
        }
    }
}

fn table_problem2(max_n: usize) {
    // Fix t = floor(n/3), f = floor(n/3) for a consistent benchmark
    println!("\n=== Problem 2: Positions with t=⌊n/3⌋ Toads and f=⌊n/3⌋ Frogs ===");
    println!(
        "{:<6} {:>4} {:>4} {:>15} {:>14}",
        "n", "t", "f", "positions", "time (s)"
    );
    println!("{}", "-".repeat(46));
    for n in 3..=max_n {
        let t = n / 3;
        let f = n / 3;
        let start = Instant::now();
        let positions = positions_with_counts(n, t, f);
        let elapsed = start.elapsed().as_secs_f64();
        println!(
            "{:<6} {:>4} {:>4} {:>15} {:>14.6}",
            n,
            t,
            f,
            positions.len(),
            elapsed
        );
        if elapsed > 1800.0 {
            println!("  (stopping: exceeded 30 min)");
            break;
        }
    }
}

// ─────────────────────────────────────────────────────────
// Problem 4 — BFS
// ─────────────────────────────────────────────────────────

/// BFS from `start` over the game tree.
/// Returns total nodes visited (start counts as 1).
/// Explores the full game tree — positions are NOT deduplicated,
/// so every path through the tree is counted separately.
pub fn bfs(start: Vec<Cell>) -> usize {
    let mut queue: VecDeque<Vec<Cell>> = VecDeque::new();
    queue.push_back(start);
    let mut visited = 0usize;

    while let Some(pos) = queue.pop_front() {
        visited += 1;
        for next in successors(&pos) {
            queue.push_back(next);
        }
    }
    visited
}

// ─────────────────────────────────────────────────────────
// Problem 5 — DFS
// ─────────────────────────────────────────────────────────

/// DFS from `start` over the game tree.
/// Returns total nodes visited (start counts as 1).
pub fn dfs(start: Vec<Cell>) -> usize {
    let mut stack: Vec<Vec<Cell>> = vec![start];
    let mut visited = 0usize;

    while let Some(pos) = stack.pop() {
        visited += 1;
        for next in successors(&pos) {
            stack.push(next);
        }
    }
    visited
}

// ─────────────────────────────────────────────────────────
// Main — demonstration
// ─────────────────────────────────────────────────────────

fn main() {
    // ── Problem 1 demo ──────────────────────────────────
    println!("=== Problem 1: all_positions(3) ===");
    let p1 = all_positions(3);
    for pos in p1.iter().take(10) {
        println!("  {}", display(pos));
    }
    println!("  ... total: {}", p1.len());

    // ── Problem 2 demo ──────────────────────────────────
    println!("\n=== Problem 2: positions_with_counts(5, 2, 2) ===");
    let p2 = positions_with_counts(5, 2, 2);
    for pos in &p2 {
        println!("  {}", display(pos));
    }
    println!("  total: {}", p2.len());

    // Cap at n=16 — n=17 requires ~1.3 GB and tends to OOM on typical machines.
    table_problem1(16);
    table_problem2(19);

    // ── Problems 4 & 5 — BFS vs DFS table ───────────────
    println!("\n=== Problems 4 & 5: BFS vs DFS node counts ===");
    println!(
        "{:<34} {:>12} {:>8} {:>12} {:>8}",
        "start position", "BFS nodes", "BFS(s)", "DFS nodes", "DFS(s)"
    );
    println!("{}", "-".repeat(76));

    let test_cases: Vec<Vec<Cell>> = vec![
        vec![Toad, Empty, Frog],
        vec![Toad, Toad, Empty, Frog, Frog],
        vec![Toad, Toad, Toad, Empty, Frog, Frog, Frog],
        vec![Toad, Toad, Toad, Toad, Empty, Frog, Frog, Frog, Frog],
        // mixed / non-standard starts
        vec![Toad, Frog, Empty, Toad, Frog],
        vec![Empty, Toad, Frog, Toad, Empty, Frog],
    ];

    for pos in test_cases {
        let label = display(&pos);

        let t0 = Instant::now();
        let b = bfs(pos.clone());
        let bt = t0.elapsed().as_secs_f64();

        let t0 = Instant::now();
        let d = dfs(pos.clone());
        let dt = t0.elapsed().as_secs_f64();

        println!("{:<34} {:>12} {:>8.6} {:>12} {:>8.6}", label, b, bt, d, dt);
    }
}

