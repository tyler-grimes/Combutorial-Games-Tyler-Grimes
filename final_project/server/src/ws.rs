use crate::protocol::{ClientMsg, ServerMsg};
use crate::room::{PlayerSlot, Room};
use crate::state::SharedState;
use crate::bot;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use std::time::Instant;
use tokio::sync::broadcast;
use uuid::Uuid;

struct Membership {
    code: String,
    seat: u8,
    gen: u64,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<SharedState>) -> Response {
    ws.on_upgrade(|sock| handle_socket(sock, state))
}

async fn handle_socket(mut sock: WebSocket, state: SharedState) {
    let mut room_rx: Option<broadcast::Receiver<ServerMsg>> = None;
    let mut lobby_rx = state.lobby_tx.subscribe();
    let mut me: Option<Membership> = None;

    loop {
        tokio::select! {
            m = sock.recv() => match m {
                Some(Ok(Message::Text(t))) => {
                    let Ok(msg) = serde_json::from_str::<ClientMsg>(&t) else {
                        break;
                    };
                    if let Some(reply) = handle_msg(msg, &state, &mut me, &mut room_rx) {
                        let txt = serde_json::to_string(&reply).unwrap();
                        if sock.send(Message::Text(txt.into())).await.is_err() {
                            break;
                        }
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => break,
            },
            r = async { room_rx.as_mut().unwrap().recv().await }, if room_rx.is_some() => {
                match r {
                    Ok(msg) => {
                        let txt = serde_json::to_string(&msg).unwrap();
                        if sock.send(Message::Text(txt.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            },
            l = lobby_rx.recv(), if me.is_none() => {
                if let Ok(msg) = l {
                    let txt = serde_json::to_string(&msg).unwrap();
                    if sock.send(Message::Text(txt.into())).await.is_err() {
                        break;
                    }
                }
            },
        }
    }

    if let Some(m) = &me {
        let mut rooms = state.rooms.lock().unwrap();
        if let Some(room) = rooms.get_mut(&m.code) {
            if m.seat == 255 {
                room.spectators = room.spectators.saturating_sub(1);
            } else if let Some(slot) = room.players[m.seat as usize].as_mut() {
                if slot.gen == m.gen {
                    slot.connected = false;
                }
            }
        }
    }
}

fn handle_msg(
    msg: ClientMsg,
    state: &SharedState,
    me: &mut Option<Membership>,
    room_rx: &mut Option<broadcast::Receiver<ServerMsg>>,
) -> Option<ServerMsg> {
    match msg {
        ClientMsg::ListRooms => Some(ServerMsg::RoomList { rooms: state.room_list() }),

        ClientMsg::CreateRoom { name, vs_bot, bot_first, local } => {
            let code = state.new_room_code();
            let mut room = Room::new(code.clone());
            let token = Uuid::new_v4().to_string();
            let human_seat = if vs_bot && bot_first { 1 } else { 0 };
            if vs_bot {
                room.bot = Some(if bot_first { 0 } else { 1 });
            }
            room.players[human_seat] =
                Some(PlayerSlot { name: name.clone(), token: token.clone(), connected: true, gen: 0 });
            if local {
                room.local = true;
                let other = 1 - human_seat;
                room.players[other] = Some(PlayerSlot {
                    name: format!("{name} (P2)"),
                    token: String::new(),
                    connected: true,
                    gen: 0,
                });
            }
            *room_rx = Some(room.tx.subscribe());
            let snapshot = room.snapshot();
            state.rooms.lock().unwrap().insert(code.clone(), room);
            *me = Some(Membership { code: code.clone(), seat: human_seat as u8, gen: 0 });
            state.push_lobby_update();
            if vs_bot && bot_first {
                bot::spawn_bot_move(state.clone(), code.clone());
            }
            Some(ServerMsg::Joined { token, seat: human_seat as u8, code, state: snapshot })
        }

        ClientMsg::JoinRoom { code, name, token } => {
            let mut rooms = state.rooms.lock().unwrap();
            let Some(room) = rooms.get_mut(&code) else {
                return Some(ServerMsg::Error { msg: "no such room".into() });
            };
            if let Some(tok) = token {
                let found = room.players.iter().position(|p| {
                    p.as_ref().is_some_and(|p| p.token == tok)
                });
                if let Some(seat) = found {
                    let slot = room.players[seat].as_mut().unwrap();
                    slot.connected = true;
                    slot.gen += 1;
                    let gen = slot.gen;
                    *room_rx = Some(room.tx.subscribe());
                    let snapshot = room.snapshot();
                    *me = Some(Membership { code: code.clone(), seat: seat as u8, gen });
                    return Some(ServerMsg::Joined { token: tok, seat: seat as u8, code, state: snapshot });
                }
            }
            let free = (0..2).find(|&s| room.players[s].is_none() && room.bot != Some(s as u8));
            let Some(seat) = free else {
                return Some(ServerMsg::Error { msg: "room full".into() });
            };
            let token = Uuid::new_v4().to_string();
            room.players[seat] =
                Some(PlayerSlot { name, token: token.clone(), connected: true, gen: 0 });
            room.last_active = Instant::now();
            *room_rx = Some(room.tx.subscribe());
            let snapshot = room.snapshot();
            room.broadcast(ServerMsg::State { state: snapshot.clone() });
            *me = Some(Membership { code: code.clone(), seat: seat as u8, gen: 0 });
            drop(rooms);
            state.push_lobby_update();
            Some(ServerMsg::Joined { token, seat: seat as u8, code, state: snapshot })
        }

        ClientMsg::Spectate { code } => {
            let mut rooms = state.rooms.lock().unwrap();
            let Some(room) = rooms.get_mut(&code) else {
                return Some(ServerMsg::Error { msg: "no such room".into() });
            };
            room.spectators += 1;
            *room_rx = Some(room.tx.subscribe());
            let snapshot = room.snapshot();
            *me = Some(Membership { code: code.clone(), seat: 255, gen: 0 });
            Some(ServerMsg::Joined { token: String::new(), seat: 255, code, state: snapshot })
        }

        ClientMsg::Move { col } => handle_move(col, state, me.as_ref()?),

        ClientMsg::Rematch => handle_rematch(state, me.as_ref()?),
    }
}

fn handle_move(col: usize, state: &SharedState, me: &Membership) -> Option<ServerMsg> {
    if me.seat > 1 {
        return Some(ServerMsg::Error { msg: "spectators can't move".into() });
    }
    let mut rooms = state.rooms.lock().unwrap();
    let Some(room) = rooms.get_mut(&me.code) else {
        return Some(ServerMsg::Error { msg: "room gone".into() });
    };
    let live = room.players[me.seat as usize].as_ref().is_some_and(|p| p.gen == me.gen);
    if !live {
        return Some(ServerMsg::Error { msg: "stale session".into() });
    }
    if room.status() != "playing" {
        return Some(ServerMsg::Error { msg: "game not in progress".into() });
    }
    if !room.local && room.seat_to_move() != me.seat {
        return Some(ServerMsg::Error { msg: "not your turn".into() });
    }
    let by = room.game.turn();
    if let Err(e) = room.game.play(col) {
        return Some(ServerMsg::Error { msg: e.into() });
    }
    room.last_active = Instant::now();
    room.broadcast(ServerMsg::State { state: room.snapshot() });
    if room.game.over {
        room.broadcast(ServerMsg::GameOver {
            winner: room.game.winner,
            line: room.game.line.clone(),
        });
        drop(rooms);
        state.push_lobby_update();
        return None;
    }
    let pos = room.game.pos;
    let bot_turn = room.bot == Some(room.seat_to_move());
    let code = me.code.clone();
    drop(rooms);
    spawn_eval(state.clone(), code.clone(), col, by, pos);
    if bot_turn {
        bot::spawn_bot_move(state.clone(), code);
    }
    None
}

fn handle_rematch(state: &SharedState, me: &Membership) -> Option<ServerMsg> {
    if me.seat > 1 {
        return Some(ServerMsg::Error { msg: "spectators can't vote".into() });
    }
    let mut rooms = state.rooms.lock().unwrap();
    let Some(room) = rooms.get_mut(&me.code) else {
        return Some(ServerMsg::Error { msg: "room gone".into() });
    };
    if !room.game.over {
        return Some(ServerMsg::Error { msg: "game not over".into() });
    }
    room.rematch_votes[me.seat as usize] = true;
    // In local pass-and-play one client owns both seats, so a single vote rematches.
    let all_voted = room.local
        || (0..2).all(|s| {
            room.bot == Some(s as u8) || room.rematch_votes[s] || room.players[s].is_none()
        });
    if all_voted {
        room.rematch();
        room.last_active = Instant::now();
        room.broadcast(ServerMsg::State { state: room.snapshot() });
        if room.bot == Some(room.seat_to_move()) {
            let code = me.code.clone();
            drop(rooms);
            bot::spawn_bot_move(state.clone(), code);
        }
    }
    None
}

fn spawn_eval(state: SharedState, code: String, col: usize, by: u8, pos: engine::Position) {
    if !state.engine.eval_available(&pos) {
        return;
    }
    tokio::spawn(async move {
        let engine = state.engine.clone();
        let Ok(eval) = tokio::task::spawn_blocking(move || engine.eval_text(&pos)).await else {
            return;
        };
        let rooms = state.rooms.lock().unwrap();
        if let Some(room) = rooms.get(&code) {
            room.broadcast(ServerMsg::MovePlayed { col, by, eval: Some(eval) });
        }
    });
}
