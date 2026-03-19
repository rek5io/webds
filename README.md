# WebDS – Web Desktop Share

**WebDS (Web Desktop Share)** is a lightweight desktop sharing application written in Rust that enables users to stream or share their desktop directly through a web interface. It is designed to be fast and simple, requiring minimal setup.

WebDS uses H.264 video encoding for efficient, high-quality compression and low-latency streaming directly in the browser, eliminating the need for additional client software.

---

## ⚠️ Platform Support

Currently **WebDS only supports Linux systems running Wayland**.

Requirements:

* Linux
* Wayland compositor
* Browser with h264 decoder

---

## Installation

### Clone the repository

```
git clone https://github.com/rek5io/webds.git
cd webds
```

### Run

```
cargo run --release
```

---

## Usage

1. Start the WebDS server.
2. Open your web browser.
3. Navigate to the local server address (for example):

```
http://localhost:3000
```
