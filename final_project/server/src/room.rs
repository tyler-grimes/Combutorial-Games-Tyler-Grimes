use crate::protocol::{GameState, ServerMsg};
use engine::{Position, HEIGHT, WIDTH};
use std::time::Instant;
use tokio::sync::broadcast;

pub struct Game {
    pub pos: Position,
    pub over: bool,
    pub winner: u8,
    pub line: Vec<[usize; 2]>,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub fn new() -> Self {
        Game { pos: Position::new(), over: false, winner: 0, line: Vec::new() }
    }

    pub fn turn(&self) -> u8 {
        if self.pos.moves().is_multiple_of(2) { 1 } else { 2 }
    }

    pub fn play(&mut self, col: usize) -> Result<(), &'static str> {
        if self.over {
            return Err("game over");
        }
        if col >= WIDTH || !self.pos.can_play(col) {
            return Err("illegal move");
        }
        let mover = self.turn();
        let won = self.pos.is_winning_move(col);
        self.pos.play(col);
        if won {
            self.over = true;
            self.winner = mover;
            self.line = find_line(&self.pos.to_grid(), mover);
        } else if self.pos.moves() as usize == WIDTH * HEIGHT {
            self.over = true; // draw
        }
        Ok(())
    }

    pub fn state(&self, names: [String; 2], status: &str) -> GameState {
        let g = self.pos.to_grid();
        GameState {
            board: g.iter().map(|row| row.to_vec()).collect(),
            turn: self.turn(),
            status: status.into(),
            names,
            winner: self.winner,
            line: self.line.clone(),
            p1_seat: 0, // Room::snapshot overwrites with the real value
        }
    }
}

fn find_line(g: &[[u8; WIDTH]; HEIGHT], player: u8) -> Vec<[usize; 2]> {
    let dirs: [(isize, isize); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
    for row in 0..HEIGHT as isize {
        for col in 0..WIDTH as isize {
            for (dc, dr) in dirs {
                let cells: Vec<[usize; 2]> = (0..4)
                    .map(|i| (col + i * dc, row + i * dr))
                    .filter(|&(c, r)| {
                        (0..WIDTH as isize).contains(&c)
                            && (0..HEIGHT as isize).contains(&r)
                            && g[r as usize][c as usize] == player
                    })
                    .map(|(c, r)| [c as usize, r as usize])
                    .collect();
                if cells.len() == 4 {
                    return cells;
                }
            }
        }
    }
    Vec::new()
}

pub struct PlayerSlot {
    pub name: String,
    pub token: String,
    pub connected: bool,
    pub gen: u64,
}

pub struct Room {
    pub code: String,
    pub game: Game,
    pub players: [Option<PlayerSlot>; 2],
    pub bot: Option<u8>,
    pub spectators: usize,
    pub tx: broadcast::Sender<ServerMsg>,
    pub last_active: Instant,
    pub rematch_votes: [bool; 2],
    pub p1_seat: u8,
    pub local: bool,
}

impl Room {
    pub fn new(code: String) -> Self {
        let (tx, _) = broadcast::channel(64);
        Room {
            code,
            game: Game::new(),
            players: [None, None],
            bot: None,
            spectators: 0,
            tx,
            last_active: Instant::now(),
            rematch_votes: [false, false],
            p1_seat: 0,
            local: false,
        }
    }

    pub fn names(&self) -> [String; 2] {
        let n = |s: usize| {
            self.players[s]
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| if self.bot == Some(s as u8) { "Bot".into() } else { "—".into() })
        };
        if self.p1_seat == 0 { [n(0), n(1)] } else { [n(1), n(0)] }
    }

    pub fn status(&self) -> &'static str {
        if self.game.over {
            "over"
        } else if self.seat_filled(0) && self.seat_filled(1) {
            "playing"
        } else {
            "waiting"
        }
    }

    fn seat_filled(&self, s: usize) -> bool {
        self.players[s].is_some() || self.bot == Some(s as u8)
    }

    pub fn color_of_seat(&self, seat: u8) -> u8 {
        if seat == self.p1_seat { 1 } else { 2 }
    }

    pub fn seat_to_move(&self) -> u8 {
        if self.game.turn() == 1 { self.p1_seat } else { 1 - self.p1_seat }
    }

    pub fn snapshot(&self) -> GameState {
        let mut st = self.game.state(self.names(), self.status());
        st.p1_seat = self.p1_seat;
        st
    }

    pub fn broadcast(&self, msg: ServerMsg) {
        let _ = self.tx.send(msg);
    }

    pub fn rematch(&mut self) {
        self.game = Game::new();
        self.rematch_votes = [false, false];
        self.p1_seat = 1 - self.p1_seat;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_alternates_and_rejects_illegal() {
        let mut g = Game::new();
        assert!(g.play(3).is_ok());
        assert_eq!(g.turn(), 2);
        let mut h = Game::new();
        for _ in 0..3 {
            h.play(0).unwrap();
            h.play(1).unwrap();
        }
        for _ in 0..3 {
            h.play(1).unwrap();
            h.play(0).unwrap();
        }
        assert!(h.play(0).is_err()); // col 0 now has 6
    }

    #[test]
    fn win_sets_over_winner_and_line() {
        let mut g = Game::new();
        // P1 vertical in col 0: 1,2,1,2,1,2,1 → P1 wins
        for col in [0usize, 1, 0, 1, 0, 1, 0] {
            g.play(col).unwrap();
        }
        assert!(g.over);
        assert_eq!(g.winner, 1);
        assert_eq!(g.line.len(), 4);
        assert!(g.line.contains(&[0, 0]) && g.line.contains(&[0, 3]));
        assert!(g.play(2).is_err()); // game over
    }

    #[test]
    fn snapshot_reflects_board() {
        let mut g = Game::new();
        g.play(3).unwrap();
        let st = g.state(["a".into(), "b".into()], "playing");
        assert_eq!(st.board[0][3], 1);
        assert_eq!(st.turn, 2);
        assert_eq!(st.status, "playing");
    }
}
