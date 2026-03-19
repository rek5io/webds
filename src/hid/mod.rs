use std::sync::{LazyLock, Mutex, mpsc};
mod wl_backend;

static HID_SENDER: LazyLock<Mutex<mpsc::Sender<HidCommand>>> = LazyLock::new(|| {
    let (s, r) = mpsc::channel::<HidCommand>();

    std::thread::spawn(move || {
        log::debug!("starting hid backend");

        let mut backend = wl_backend::WlBackend::new();

        loop {
            if let Ok(event) = r.recv() {
                //log::debug!("processing event: {:?}", event.0);
                event.execute(&mut backend);
            }
        }
    });

    Mutex::new(s)
});

pub fn send_event(event: HidCommand) {
    HID_SENDER.lock().unwrap().send(event).unwrap();
}

pub struct HidCommand(String);

impl HidCommand {
    pub fn new(data: impl ToString) -> Self {
        Self(data.to_string())
    }

    fn execute(self, backend: &mut wl_backend::WlBackend) {
        if let Some(cords) = self.0.strip_prefix("mma") {
            if let Some((x, y)) = cords.trim().split_once(' ') {
                let x = x.parse::<f64>().unwrap_or_default();
                let y = y.parse::<f64>().unwrap_or_default();
                backend.mouse_move(x, y);
            }
        }

        if let Some(btn) = self.0.strip_prefix("pm") {
            if let Ok(btn) = btn.trim().parse::<u8>() {
                backend.mouse_press(btn);
            }
        }

        if let Some(btn) = self.0.strip_prefix("rm") {
            if let Ok(btn) = btn.trim().parse::<u8>() {
                backend.mouse_release(btn);
            }
        }

        if let Some(key) = self.0.strip_prefix("pk") {
            let _ = backend.press_key(key.trim());
        }

        if let Some(key) = self.0.strip_prefix("rk") {
            let _ = backend.release_key(key.trim());
        }
    }
}
