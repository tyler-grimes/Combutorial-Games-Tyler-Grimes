use std::time::Duration;

#[tokio::main]
async fn main() {
    let book_path = std::path::Path::new("engine/book.bin");
    let engine = engine::Engine::new(book_path.exists().then_some(book_path));
    if book_path.exists() {
        println!("opening book loaded from {}", book_path.display());
    } else {
        eprintln!("WARNING: no engine/book.bin — bot/eval will be slow on early moves");
    }
    let state = server::state::AppState::new(engine);

    let reaper = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            let removed = {
                let mut rooms = reaper.rooms.lock().unwrap();
                let before = rooms.len();
                rooms.retain(|_, r| r.last_active.elapsed() < Duration::from_secs(3600));
                before - rooms.len()
            };
            if removed > 0 {
                reaper.push_lobby_update();
            }
        }
    });

    let app = server::app(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Connect-4 server: http://<your-LAN-IP>:3000");
    axum::serve(listener, app).await.unwrap();
}
