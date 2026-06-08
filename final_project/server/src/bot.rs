use crate::protocol::ServerMsg;
use crate::state::SharedState;
use std::time::{Duration, Instant};

pub fn spawn_bot_move(state: SharedState, code: String) {
    tokio::spawn(async move {
        let pos = {
            let rooms = state.rooms.lock().unwrap();
            let Some(room) = rooms.get(&code) else { return };
            if room.game.over || room.bot != Some(room.seat_to_move()) {
                return;
            }
            room.game.pos
        };

        let engine = state.engine.clone();
        let result = tokio::task::spawn_blocking(move || {
            let col = engine.best_move(&pos);
            let eval = if pos.is_winning_move(col) {
                None
            } else {
                let mut after = pos;
                after.play(col);
                engine.eval_available(&after).then(|| engine.eval_text(&after))
            };
            (col, eval)
        })
        .await;

        tokio::time::sleep(Duration::from_secs(1)).await;

        let mut rooms = state.rooms.lock().unwrap();
        let Some(room) = rooms.get_mut(&code) else { return };

        let Ok((col, eval)) = result else {
            room.game.over = true;
            room.game.winner = 3 - room.game.turn();
            room.broadcast(ServerMsg::Error { msg: "bot crashed — game forfeited".into() });
            room.broadcast(ServerMsg::GameOver { winner: room.game.winner, line: vec![] });
            return;
        };

        if room.game.pos != pos {
            return;
        }
        let by = room.game.turn();
        if room.game.play(col).is_err() {
            return;
        }
        room.last_active = Instant::now();
        room.broadcast(ServerMsg::State { state: room.snapshot() });
        room.broadcast(ServerMsg::MovePlayed { col, by, eval });
        if room.game.over {
            room.broadcast(ServerMsg::GameOver {
                winner: room.game.winner,
                line: room.game.line.clone(),
            });
        }
    });
}
