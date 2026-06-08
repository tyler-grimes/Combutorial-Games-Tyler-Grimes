use crate::protocol::{RoomInfo, ServerMsg};
use crate::room::Room;
use engine::Engine;
use rand::RngExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

pub struct AppState {
    pub rooms: Mutex<HashMap<String, Room>>,
    pub engine: Arc<Engine>,
    pub lobby_tx: broadcast::Sender<ServerMsg>,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(engine: Engine) -> SharedState {
        let (lobby_tx, _) = broadcast::channel(64);
        Arc::new(AppState {
            rooms: Mutex::new(HashMap::new()),
            engine: Arc::new(engine),
            lobby_tx,
        })
    }

    pub fn room_list(&self) -> Vec<RoomInfo> {
        self.rooms
            .lock()
            .unwrap()
            .values()
            .map(|r| RoomInfo {
                code: r.code.clone(),
                host: r.players[0]
                    .as_ref()
                    .or(r.players[1].as_ref())
                    .map(|p| p.name.clone())
                    .unwrap_or_default(),
                vs_bot: r.bot.is_some(),
                open: (0..2).any(|s| {
                    r.players[s].is_none() && r.bot != Some(s as u8)
                }),
            })
            .collect()
    }

    pub fn push_lobby_update(&self) {
        let _ = self.lobby_tx.send(ServerMsg::RoomList { rooms: self.room_list() });
    }

    pub fn new_room_code(&self) -> String {
        let rooms = self.rooms.lock().unwrap();
        let mut rng = rand::rng();
        loop {
            let code: String = (0..4)
                .map(|_| rng.random_range(b'A'..=b'Z') as char)
                .collect();
            if !rooms.contains_key(&code) {
                return code;
            }
        }
    }
}
