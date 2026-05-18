use std::os::fd::AsFd;
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle,
    protocol::{
        wl_keyboard::{KeyState, KeymapFormat},
        wl_output::{self, WlOutput},
        wl_pointer::ButtonState,
        wl_registry,
        wl_seat::WlSeat,
    },
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};
use xkbcommon::xkb;

pub struct WlBackend {
    keyboard: ZwpVirtualKeyboardV1,
    keymap: xkb::Keymap,
    pointer: ZwlrVirtualPointerV1,
    event_queue: EventQueue<State>,
    width: f64,
    height: f64,
}

struct State {
    output: Option<WlOutput>,
    seat: Option<WlSeat>,
    keyboard_mgr: Option<ZwpVirtualKeyboardManagerV1>,
    pointer_mgr: Option<ZwlrVirtualPointerManagerV1>,
    width: f64,
    height: f64,
}

impl WlBackend {
    pub fn new() -> Self {
        let mut state = State {
            output: None,
            seat: None,
            keyboard_mgr: None,
            pointer_mgr: None,
            width: f64::NAN,
            height: f64::NAN,
        };

        let conn = Connection::connect_to_env().unwrap();
        let display = conn.display();
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();
        let _registry = display.get_registry(&qh, ());

        event_queue.roundtrip(&mut state).unwrap();
        event_queue.roundtrip(&mut state).unwrap();

        let keyboard = state
            .keyboard_mgr
            .as_ref()
            .unwrap()
            .create_virtual_keyboard(state.seat.as_ref().unwrap(), &qh, ());

        let pointer = state.pointer_mgr.as_ref().unwrap().create_virtual_pointer(
            state.seat.as_ref(),
            &qh,
            (),
        );

        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);

        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .unwrap();

        let keymap_string = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
        let size = keymap_string.len() + 1; // null term

        let fd = memfd::MemfdOptions::new()
            .create("virtual_keyboard_keymap")
            .unwrap();

        fd.as_file().set_len(size as u64).unwrap();
        std::io::Write::write_all(&mut fd.as_file(), keymap_string.as_bytes()).unwrap();

        keyboard.keymap(
            KeymapFormat::XkbV1 as u32,
            fd.as_file().as_fd(),
            size as u32,
        );

        event_queue.flush().unwrap();

        if !state.width.is_normal() || !state.height.is_normal() {
            panic!("bad width or height, got: {}x{}", state.width, state.height);
        }

        Self {
            keyboard,
            keymap,
            pointer,
            event_queue,
            width: state.width,
            height: state.height,
        }
    }

    pub fn press_key(&mut self, key_name: &str) {
        let _ = &self.keymap;
        let keycode = Self::keyname_to_keycode(key_name);
        self.keyboard.key(0, keycode, KeyState::Pressed.into());
        self.event_queue.flush().unwrap();
    }

    pub fn release_key(&mut self, key_name: &str) {
        let keycode = Self::keyname_to_keycode(key_name);
        self.keyboard.key(0, keycode, KeyState::Released.into());
        self.event_queue.flush().unwrap();
    }

    pub fn mouse_move(&mut self, x: f64, y: f64) {
        self.pointer.motion_absolute(
            0,
            (self.width * x) as u32,
            (self.height * y) as u32,
            self.width as u32,
            self.height as u32,
        );
        self.event_queue.flush().unwrap();
    }

    pub fn mouse_press(&mut self, button: u8) {
        self.pointer
            .button(0, Self::get_mouse_button(button), ButtonState::Pressed);
        self.event_queue.flush().unwrap();
    }

    pub fn mouse_release(&mut self, button: u8) {
        self.pointer
            .button(0, Self::get_mouse_button(button), ButtonState::Released);
        self.event_queue.flush().unwrap();
    }

    fn get_mouse_button(button: u8) -> u32 {
        match button {
            0 => 0x110,
            1 => 0x111,
            2 => 0x112,
            _ => 0,
        }
    }

    fn keyname_to_keycode(key_name: &str) -> u32 {
        match key_name.to_ascii_lowercase().as_str() {
            "a" => 30,
            "b" => 48,
            "c" => 46,
            "d" => 32,
            "e" => 18,
            "f" => 33,
            "g" => 34,
            "h" => 35,
            "i" => 23,
            "j" => 36,
            "k" => 37,
            "l" => 38,
            "m" => 50,
            "n" => 49,
            "o" => 24,
            "p" => 25,
            "q" => 16,
            "r" => 19,
            "s" => 31,
            "t" => 20,
            "u" => 22,
            "v" => 47,
            "w" => 17,
            "x" => 45,
            "y" => 21,
            "z" => 44,

            "1" => 2,
            "2" => 3,
            "3" => 4,
            "4" => 5,
            "5" => 6,
            "6" => 7,
            "7" => 8,
            "8" => 9,
            "9" => 10,
            "0" => 11,

            "numpad0" => 82,
            "numpad1" => 79,
            "numpad2" => 80,
            "numpad3" => 81,
            "numpad4" => 75,
            "numpad5" => 76,
            "numpad6" => 77,
            "numpad7" => 71,
            "numpad8" => 72,
            "numpad9" => 73,
            "numpadenter" => 96,
            "numpadadd" => 78,
            "numpadsubtract" => 74,
            "numpadmultiply" => 55,
            "numpaddivide" => 98,
            "numpaddecimal" => 83,

            "-" => 12,
            "=" => 13,
            "[" => 26,
            "]" => 27,
            "\\" => 43,
            ";" => 39,
            "'" => 40,
            "`" => 41,
            "," => 51,
            "." => 52,
            "/" => 53,

            "!" => 2,
            "@" => 3,
            "#" => 4,
            "$" => 5,
            "%" => 6,
            "^" => 7,
            "&" => 8,
            "*" => 9,
            "(" => 10,
            ")" => 11,
            "_" => 12,
            "+" => 13,
            "{" => 26,
            "}" => 27,
            "|" => 43,
            ":" => 39,
            "\"" => 40,
            "~" => 41,
            "<" => 51,
            ">" => 52,
            "?" => 53,

            "shift" | "shiftleft" => 42,
            "shiftright" => 54,
            "control" | "controlleft" => 29,
            "controlright" => 97,
            "alt" | "altleft" => 56,
            "altright" => 100,
            "meta" | "metaleft" => 125,
            "metaright" => 126,
            "capslock" => 58,

            "insert" => 110,
            "delete" => 111,
            "home" => 102,
            "end" => 107,
            "pageup" => 104,
            "pagedown" => 109,

            "arrowdown" => 108,
            "arrowup" => 103,
            "arrowleft" => 105,
            "arrowright" => 106,

            "f1" => 59,
            "f2" => 60,
            "f3" => 61,
            "f4" => 62,
            "f5" => 63,
            "f6" => 64,
            "f7" => 65,
            "f8" => 66,
            "f9" => 67,
            "f10" => 68,
            "f11" => 87,
            "f12" => 88,

            "printscreen" => 99,
            "scrolllock" => 70,
            "pause" => 119,
            " " | "space" => 57,
            "enter" => 28,
            "tab" => 15,
            "backspace" => 14,
            "escape" => 1,

            _ => 0,
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<State>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "zwp_virtual_keyboard_manager_v1" => {
                    state.keyboard_mgr = Some(registry.bind::<ZwpVirtualKeyboardManagerV1, _, _>(
                        name,
                        version,
                        qh,
                        (),
                    ));
                }

                "zwlr_virtual_pointer_manager_v1" => {
                    state.pointer_mgr = Some(registry.bind::<ZwlrVirtualPointerManagerV1, _, _>(
                        name,
                        version,
                        qh,
                        (),
                    ));
                }

                "wl_seat" => {
                    state.seat = Some(registry.bind::<WlSeat, _, _>(name, version, qh, ()));
                }

                "wl_output" => {
                    state.output = Some(registry.bind::<WlOutput, _, _>(name, version, qh, ()));
                }

                _ => {}
            },

            _ => {}
        }
    }
}

impl Dispatch<WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &WlOutput,
        event: <WlOutput as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Mode { width, height, .. } => {
                state.width = width as f64;
                state.height = height as f64;
            }
            _ => {}
        }
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WlSeat,
        _event: <WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardManagerV1,
        _event: <ZwpVirtualKeyboardManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardV1,
        _event: <ZwpVirtualKeyboardV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrVirtualPointerManagerV1,
        _event: <ZwlrVirtualPointerManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrVirtualPointerV1,
        _event: <ZwlrVirtualPointerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}
