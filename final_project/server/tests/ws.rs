use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn spawn_app() -> String {
    let state = server::state::AppState::new(engine::Engine::new(None));
    let app = server::app(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("ws://{addr}/ws")
}

async fn ws(url: &str) -> Ws {
    connect_async(url).await.unwrap().0
}

async fn send(w: &mut Ws, v: Value) {
    w.send(Message::Text(v.to_string().into())).await.unwrap();
}

async fn recv(w: &mut Ws) -> Value {
    loop {
        let m = tokio::time::timeout(std::time::Duration::from_secs(5), w.next())
            .await
            .expect("recv timeout")
            .expect("socket closed")
            .unwrap();
        if let Message::Text(t) = m {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

async fn recv_type(w: &mut Ws, ty: &str) -> Value {
    loop {
        let v = recv(w).await;
        if v["type"] == ty {
            return v;
        }
    }
}

#[tokio::test]
async fn create_and_list_rooms() {
    let url = spawn_app().await;
    let mut a = ws(&url).await;
    send(&mut a, json!({"type":"CreateRoom","name":"alice","vs_bot":false})).await;
    let joined = recv_type(&mut a, "Joined").await;
    assert_eq!(joined["seat"], 0);
    let code = joined["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 4);
    assert!(!joined["token"].as_str().unwrap().is_empty());
    assert_eq!(joined["state"]["status"], "waiting");

    let mut b = ws(&url).await;
    send(&mut b, json!({"type":"ListRooms"})).await;
    let list = recv_type(&mut b, "RoomList").await;
    assert_eq!(list["rooms"][0]["code"], code.as_str());
    assert_eq!(list["rooms"][0]["open"], true);
}

#[tokio::test]
async fn join_unknown_room_errors() {
    let url = spawn_app().await;
    let mut a = ws(&url).await;
    send(&mut a, json!({"type":"JoinRoom","code":"ZZZZ","name":"bob"})).await;
    let e = recv_type(&mut a, "Error").await;
    assert!(e["msg"].as_str().unwrap().contains("room"));
}

#[tokio::test]
async fn second_join_starts_game() {
    let url = spawn_app().await;
    let mut a = ws(&url).await;
    send(&mut a, json!({"type":"CreateRoom","name":"alice","vs_bot":false})).await;
    let code = recv_type(&mut a, "Joined").await["code"].as_str().unwrap().to_string();

    let mut b = ws(&url).await;
    send(&mut b, json!({"type":"JoinRoom","code":code,"name":"bob"})).await;
    let joined = recv_type(&mut b, "Joined").await;
    assert_eq!(joined["seat"], 1);
    assert_eq!(joined["state"]["status"], "playing");
    let st = recv_type(&mut a, "State").await;
    assert_eq!(st["state"]["status"], "playing");
}

/// A creates, B joins; returns (a, b, code, token_a).
/// Both A and B queues are empty on return (start-of-game State drained from both).
async fn start_game(url: &str) -> (Ws, Ws, String, String) {
    let mut a = ws(url).await;
    send(&mut a, json!({"type":"CreateRoom","name":"alice","vs_bot":false})).await;
    let j = recv_type(&mut a, "Joined").await;
    let code = j["code"].as_str().unwrap().to_string();
    let token_a = j["token"].as_str().unwrap().to_string();
    let mut b = ws(url).await;
    send(&mut b, json!({"type":"JoinRoom","code":code,"name":"bob"})).await;
    recv_type(&mut b, "Joined").await;
    recv_type(&mut a, "State").await; // status flip broadcast — drain from a
    recv_type(&mut b, "State").await; // status flip broadcast — drain from b
    (a, b, code, token_a)
}

#[tokio::test]
async fn full_game_vertical_win() {
    let url = spawn_app().await;
    let (mut a, mut b, _, _) = start_game(&url).await;
    // A: col 0 ×4, B: col 1 ×3 → A wins
    for (i, col) in [0usize, 1, 0, 1, 0, 1, 0].iter().enumerate() {
        let w: &mut Ws = if i % 2 == 0 { &mut a } else { &mut b };
        send(w, json!({"type":"Move","col":col})).await;
        recv_type(&mut a, "State").await;
        recv_type(&mut b, "State").await;
    }
    let over = recv_type(&mut a, "GameOver").await;
    assert_eq!(over["winner"], 1);
    assert_eq!(over["line"].as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn illegal_moves_rejected() {
    let url = spawn_app().await;
    let (mut a, mut b, _, _) = start_game(&url).await;
    // not B's turn
    send(&mut b, json!({"type":"Move","col":3})).await;
    let e = recv_type(&mut b, "Error").await;
    assert_eq!(e["msg"], "not your turn");
    // bad column
    send(&mut a, json!({"type":"Move","col":9})).await;
    let e = recv_type(&mut a, "Error").await;
    assert_eq!(e["msg"], "illegal move");
    // board unchanged: A can still play col 3 fine
    send(&mut a, json!({"type":"Move","col":3})).await;
    let st = recv_type(&mut a, "State").await;
    assert_eq!(st["state"]["board"][0][3], 1);
}

#[tokio::test]
async fn reconnect_reclaims_seat() {
    let url = spawn_app().await;
    let (mut a, mut b, code, token_a) = start_game(&url).await;
    send(&mut a, json!({"type":"Move","col":3})).await;
    recv_type(&mut b, "State").await;
    drop(a);

    let mut a2 = ws(&url).await;
    send(&mut a2, json!({"type":"JoinRoom","code":code,"name":"alice","token":token_a})).await;
    let j = recv_type(&mut a2, "Joined").await;
    assert_eq!(j["seat"], 0);
    assert_eq!(j["state"]["board"][0][3], 1);

    send(&mut b, json!({"type":"Move","col":3})).await;
    recv_type(&mut a2, "State").await;
    recv_type(&mut b, "State").await;
    send(&mut a2, json!({"type":"Move","col":2})).await;
    let st = recv_type(&mut b, "State").await;
    assert_eq!(st["state"]["board"][0][2], 1);
}

#[tokio::test]
async fn rematch_swaps_starting_player() {
    let url = spawn_app().await;
    let (mut a, mut b, _, _) = start_game(&url).await;
    for (i, col) in [0usize, 1, 0, 1, 0, 1, 0].iter().enumerate() {
        let w: &mut Ws = if i % 2 == 0 { &mut a } else { &mut b };
        send(w, json!({"type":"Move","col":col})).await;
        recv_type(&mut a, "State").await;
        recv_type(&mut b, "State").await;
    }
    // drain GameOver from both
    recv_type(&mut a, "GameOver").await;
    recv_type(&mut b, "GameOver").await;
    send(&mut a, json!({"type":"Rematch"})).await;
    send(&mut b, json!({"type":"Rematch"})).await;
    let st = recv_type(&mut a, "State").await;
    assert_eq!(st["state"]["status"], "playing");
    assert_eq!(st["state"]["names"][0], "bob"); // seats swapped: bob is P1 now
    recv_type(&mut b, "State").await; // drain rematch state from b
    // bob (P1 of game 2) moves first
    send(&mut b, json!({"type":"Move","col":3})).await;
    recv_type(&mut b, "State").await; // b sees own move
    let st = recv_type(&mut a, "State").await;
    assert_eq!(st["state"]["board"][0][3], 1);
}

#[tokio::test]
async fn local_rematch_resets_with_single_vote() {
    // In pass-and-play mode one client owns both seats, so a single Rematch
    // click must reset the board — there is no second client to vote.
    let url = spawn_app().await;
    let mut a = ws(&url).await;
    send(&mut a, json!({"type":"CreateRoom","name":"alice","vs_bot":false,"local":true})).await;
    let joined = recv_type(&mut a, "Joined").await;
    assert_eq!(joined["state"]["status"], "playing");
    // P1 vertical win in col 0; the single client plays both seats.
    for col in [0usize, 1, 0, 1, 0, 1, 0] {
        send(&mut a, json!({"type":"Move","col":col})).await;
        recv_type(&mut a, "State").await;
    }
    recv_type(&mut a, "GameOver").await;
    send(&mut a, json!({"type":"Rematch"})).await;
    let st = recv_type(&mut a, "State").await;
    assert_eq!(st["state"]["status"], "playing");
    assert_eq!(st["state"]["board"][0][0], 0, "board should be fresh after rematch");
}

#[tokio::test]
async fn spectator_sees_moves() {
    let url = spawn_app().await;
    let (mut a, mut b, code, _) = start_game(&url).await;
    let mut c = ws(&url).await;
    send(&mut c, json!({"type":"Spectate","code":code})).await;
    let j = recv_type(&mut c, "Joined").await;
    assert_eq!(j["seat"], 255);
    send(&mut a, json!({"type":"Move","col":4})).await;
    let st = recv_type(&mut c, "State").await;
    assert_eq!(st["state"]["board"][0][4], 1);
    send(&mut c, json!({"type":"Move","col":0})).await;
    let e = recv_type(&mut c, "Error").await;
    assert_eq!(e["msg"], "spectators can't move");
    let _ = recv_type(&mut b, "State").await;
}

/// Needs engine/book.bin (Task 9 step 7). Run: cargo test -p server --release -- --ignored
#[tokio::test]
#[ignore]
async fn bot_replies_and_never_hangs() {
    let book = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../engine/book.bin"));
    assert!(book.exists(), "generate engine/book.bin first (Task 9)");
    let state = server::state::AppState::new(engine::Engine::new(Some(book)));
    let app = server::app(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let url = format!("ws://{addr}/ws");

    let mut a = ws(&url).await;
    send(&mut a, json!({"type":"CreateRoom","name":"human","vs_bot":true})).await;
    recv_type(&mut a, "Joined").await;
    'outer: for col in [0usize, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4] {
        send(&mut a, json!({"type":"Move","col":col})).await;
        loop {
            let v = recv(&mut a).await;
            match v["type"].as_str().unwrap() {
                "GameOver" => {
                    assert_eq!(v["winner"], 2, "perfect bot (P2) cannot lose to this line");
                    return;
                }
                "Error" => continue 'outer,
                "MovePlayed" if v["by"] == 2 => continue 'outer,
                _ => {}
            }
        }
    }
    panic!("game never ended");
}
