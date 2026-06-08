use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ClientMsg {
    CreateRoom {
        name: String,
        vs_bot: bool,
        #[serde(default)]
        bot_first: bool,
    },
    JoinRoom {
        code: String,
        name: String,
        #[serde(default)]
        token: Option<String>,
    },
    Spectate { code: String },
    ListRooms,
    Move { col: usize },
    Rematch,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ServerMsg {
    RoomList { rooms: Vec<RoomInfo> },
    /// seat: 0 = first player (P1 of game 1), 1 = second. 255 = spectator.
    Joined { token: String, seat: u8, code: String, state: GameState },
    State { state: GameState },
    MovePlayed { col: usize, by: u8, eval: Option<String> },
    /// winner: 1|2, 0 = draw. line = winning cells as [col,row].
    GameOver { winner: u8, line: Vec<[usize; 2]> },
    Error { msg: String },
}

#[derive(Serialize, Clone, Debug)]
pub struct RoomInfo {
    pub code: String,
    pub host: String,
    pub vs_bot: bool,
    pub open: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct GameState {
    pub board: Vec<Vec<u8>>,
    pub turn: u8,
    pub status: String,
    pub names: [String; 2],
    pub winner: u8,
    pub line: Vec<[usize; 2]>,
    pub p1_seat: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_msgs_parse() {
        let m: ClientMsg =
            serde_json::from_str(r#"{"type":"CreateRoom","name":"ty","vs_bot":true}"#).unwrap();
        assert!(matches!(m, ClientMsg::CreateRoom { vs_bot: true, bot_first: false, .. }));

        let m: ClientMsg =
            serde_json::from_str(r#"{"type":"JoinRoom","code":"ABCD","name":"jo"}"#).unwrap();
        assert!(matches!(m, ClientMsg::JoinRoom { token: None, .. }));

        let m: ClientMsg = serde_json::from_str(r#"{"type":"Move","col":3}"#).unwrap();
        assert!(matches!(m, ClientMsg::Move { col: 3 }));
    }

    #[test]
    fn server_msgs_serialize_tagged() {
        let s = serde_json::to_string(&ServerMsg::Error { msg: "nope".into() }).unwrap();
        assert!(s.contains(r#""type":"Error""#));
    }

    #[test]
    fn malformed_json_is_err() {
        assert!(serde_json::from_str::<ClientMsg>(r#"{"type":"Fly"}"#).is_err());
    }
}
