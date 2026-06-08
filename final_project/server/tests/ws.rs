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
