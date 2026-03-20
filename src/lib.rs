pub mod args;
mod cap;
mod hid;

use axum::{
    Router,
    extract::{Request, ws::WebSocketUpgrade},
    response::{Html, IntoResponse, Response},
};
use hyper::body::Incoming;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower_service::Service;

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

pub fn load_tls() -> TlsAcceptor {
    let mut cert_reader = std::io::Cursor::new(include_bytes!("../cert.pem"));
    let mut key_reader = std::io::Cursor::new(include_bytes!("../key.pem"));

    let certs = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let key = rustls_pemfile::private_key(&mut key_reader)
        .unwrap()
        .unwrap();

    let config = tokio_rustls::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();

    TlsAcceptor::from(std::sync::Arc::new(config))
}

pub async fn serve(router: Router, tls_acceptor: TlsAcceptor, listener: TcpListener) -> ! {
    loop {
        let router = router.clone();
        let tls_acceptor = tls_acceptor.clone();

        let Ok((cnx, addr)) = listener.accept().await else {
            log::error!("couldn't accept connection");
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            continue;
        };

        tokio::spawn(async move {
            if !util::is_tls(&cnx).await {
                let http_service =
                    hyper::service::service_fn(|req: Request<Incoming>| async move {
                        Ok::<_, hyper::Error>(util::redirect_http_to_https(req))
                    });

                let stream = hyper_util::rt::TokioIo::new(cnx);

                let _ = hyper_util::server::conn::auto::Builder::new(
                    hyper_util::rt::TokioExecutor::new(),
                )
                .serve_connection_with_upgrades(stream, http_service)
                .await;
            } else {
                let Ok(stream) = tls_acceptor.accept(cnx).await else {
                    log::debug!("couldn't accept tls");
                    return;
                };

                let stream = hyper_util::rt::TokioIo::new(stream);

                let service = hyper::service::service_fn(move |req: Request<Incoming>| {
                    router.clone().call(req)
                });

                if let Err(err) = hyper_util::server::conn::auto::Builder::new(
                    hyper_util::rt::TokioExecutor::new(),
                )
                .serve_connection_with_upgrades(stream, service)
                .await
                {
                    log::error!("got error: {:?}, serving {}", err, addr);
                }
            }
        });
    }
}

pub mod util {
    use super::*;
    use axum::response::Redirect;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    pub async fn try_poll_once<T>(fut: impl Future<Output = T>) -> Option<T> {
        struct PollOnce<F> {
            fut: F,
        }

        impl<T, F> Future for PollOnce<F>
        where
            F: Future<Output = T>,
        {
            type Output = Option<T>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                //SAFETY: fut is not moved in memory
                let fut = unsafe { self.map_unchecked_mut(|s| &mut s.fut) };

                match fut.poll(cx) {
                    Poll::Ready(t) => Poll::Ready(Some(t)),
                    Poll::Pending => Poll::Ready(None),
                }
            }
        }

        (PollOnce { fut }).await
    }

    pub async fn is_tls(cnx: &tokio::net::TcpStream) -> bool {
        let mut buff = [0u8; 1];
        if cnx.peek(&mut buff).await.is_ok() {
            buff[0] == 0x16
        } else {
            false
        }
    }

    pub fn redirect_http_to_https(req: Request<Incoming>) -> Response {
        let host = req
            .headers()
            .get("host")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        let location = format!("https://{}{}", host, req.uri());
        log::debug!("redirect to {}", location);

        Redirect::permanent(&location).into_response()
    }
}
