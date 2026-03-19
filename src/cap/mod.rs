use axum::extract::ws;
use std::sync::{atomic, mpsc};

mod wl_backend;

pub struct NalSender {
    ws: ws::WebSocket,
}

impl NalSender {
    pub fn new(ws: ws::WebSocket) -> Self {
        Self { ws }
    }

    pub async fn try_send(&mut self, data: Vec<u8>) -> Result<(), ()> {
        if super::util::try_poll_once(self.ws.recv())
            .await
            .is_some_and(|r| r.is_none())
        {
            return Err(());
        }

        if self.ws.send(data.into()).await.is_ok() {
            Ok(())
        } else {
            Err(())
        }
    }
}

pub struct Cap {
    ns_queue: tokio::sync::Mutex<Vec<NalSender>>,
    refresh_encoder: atomic::AtomicBool,
}

impl Cap {
    fn get_instance() -> &'static Cap {
        static CAP: Cap = Cap {
            ns_queue: tokio::sync::Mutex::const_new(Vec::new()),
            refresh_encoder: atomic::AtomicBool::new(false),
        };

        &CAP
    }

    pub async fn send_ns(ns: NalSender) {
        let mut ns_queue = Self::get_instance().ns_queue.lock().await;

        if ns_queue.is_empty() {
            ns_queue.push(ns);
            Self::task_run();
            log::debug!("ns sent to new task");
        } else {
            Self::get_instance()
                .refresh_encoder
                .store(true, atomic::Ordering::SeqCst);
            ns_queue.push(ns);
            log::debug!("ns sent to existing task");
        }
    }

    fn task_run() {
        if !wl_backend::WlBackend::on_wayland() {
            panic!("only wayland is supported");
        }

        let fps = super::args::get_args().fps;

        log::debug!("cap task start, target fps: {}", fps);

        let (frame_sender, frame_receiver) = mpsc::sync_channel(1);
        let (nal_sender, nal_receiver) = mpsc::sync_channel(1);

        std::thread::spawn(move || {
            log::debug!("wl backend thread start");

            let mut backend = match wl_backend::WlBackend::new(true) {
                Ok(b) => b,
                Err(e) => {
                    log::error!("wl backend thread error: {:?}", e);
                    return;
                }
            };

            loop {
                if frame_sender.send(backend.next_frame()).is_err() {
                    break;
                }
            }

            log::debug!("wl backend thread stop");
        });

        std::thread::spawn(move || {
            log::debug!("encoder thread start");

            let cfg = openh264::encoder::EncoderConfig::new()
                .bitrate(openh264::encoder::BitRate::from_bps(50 * 1024 * 1024));

            let mut encoder = openh264::encoder::Encoder::with_api_config(
                openh264::OpenH264API::from_source(),
                cfg,
            )
            .unwrap();

            const INTRA_CNT: usize = 60;
            let mut do_intra = INTRA_CNT;

            loop {
                if Self::get_instance()
                    .refresh_encoder
                    .swap(false, atomic::Ordering::SeqCst)
                {
                    do_intra = INTRA_CNT;
                }

                if do_intra > 0 {
                    do_intra -= 1;
                    encoder.force_intra_frame();

                    if do_intra == 0 {
                        log::debug!("intra resend done");
                    }
                }

                let nal = frame_receiver
                    .recv()
                    .unwrap()
                    .map(|frame| encoder.encode(&frame).unwrap().to_vec());

                if nal_sender.send(nal).is_err() {
                    break;
                }
            }

            log::debug!("encoder thread stop");
        });

        tokio::spawn(async move {
            let mut ticker =
                tokio::time::interval(std::time::Duration::from_millis((1000 / fps) as u64));

            let mut set = tokio::task::JoinSet::new();

            loop {
                let _ = ticker.tick().await;
                let mut ns_queue = Self::get_instance().ns_queue.lock().await;

                if ns_queue.is_empty() {
                    break;
                }

                let nal = nal_receiver.recv().unwrap().unwrap();

                //let start = std::time::Instant::now();

                while let Some(mut ns) = ns_queue.pop() {
                    let nal_clone = nal.clone();

                    set.spawn(async move {
                        if ns.try_send(nal_clone).await.is_ok() {
                            Some(ns)
                        } else {
                            None
                        }
                    });
                }

                while let Some(ns) = set.join_next().await {
                    if let Ok(Some(ns)) = ns {
                        ns_queue.push(ns);
                    }
                }

                //log::debug!(
                //    "frame send took: {:?}, and send {}KB",
                //    std::time::Instant::now() - start,
                //    nal.len() / 1024
                //);
            }

            log::debug!("cap task stop");
        });
    }
}
