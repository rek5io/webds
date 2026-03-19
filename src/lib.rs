pub mod args;
mod cap;
mod hid;
pub mod util;

use axum::{
    extract::ws::WebSocketUpgrade,
    response::{Html, Response},
};

pub async fn handle_hid(wu: WebSocketUpgrade) -> Response {
    wu.on_upgrade(async |mut ws| {
        while let Some(Ok(msg)) = ws.recv().await {
            if let Ok(msg_text) = msg.into_text() {
                hid::send_event(hid::HidCommand::new(msg_text));
            }
        }
    })
}

pub async fn handle_cap(wu: WebSocketUpgrade) -> Response {
    wu.on_upgrade(async |ws| {
        cap::Cap::send_ns(cap::NalSender::new(ws)).await;
    })
}

pub async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}
