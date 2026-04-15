use memmap2::Mmap;
use openh264::formats::{RgbSliceU8, YUVBuffer};
use std::os::fd::AsFd;
use wayland_client::protocol::{
    wl_buffer::WlBuffer,
    wl_output::WlOutput,
    wl_shm::{self, WlShm},
    wl_shm_pool::WlShmPool,
};
use wayland_client::{Connection, Dispatch, QueueHandle, protocol::wl_registry};
use wayland_client::{EventQueue, WEnum};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

pub struct WlBackend {
    screencopy_mgr: Option<ZwlrScreencopyManagerV1>,
    qh: Option<QueueHandle<WlBackend>>,
    ev: Option<EventQueue<WlBackend>>,
    shm: Option<WlShm>,
    output: Option<WlOutput>,
    ready_frame: Option<YUVBuffer>,
    buffer: Option<WlBuffer>,
    rgb_buffer: Vec<u8>,
    mmap: Option<Mmap>,
    width: u32,
    height: u32,
    stride: u32,
    format: wl_shm::Format,
    show_cursor: bool,
}

#[derive(Debug)]
pub enum Error {
    Connect,
    Dispatch,
    ScreenManager,
    Output,
    Buffer,
    NoFrame,
}

impl WlBackend {
    pub fn on_wayland() -> bool {
        Connection::connect_to_env().is_ok()
    }

    pub fn new(show_cursor: bool) -> Result<Self, Error> {
        let mut instance = Self {
            screencopy_mgr: None,
            qh: None,
            ev: None,
            shm: None,
            output: None,
            ready_frame: None,
            buffer: None,
            rgb_buffer: Vec::new(),
            mmap: None,
            width: 0,
            height: 0,
            stride: 0,
            format: wl_shm::Format::Abgr1555,
            show_cursor,
        };

        let conn = Connection::connect_to_env().map_err(|_| Error::Connect)?;

        let display = conn.display();
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();
        let _registry = display.get_registry(&qh, ());

        event_queue
            .blocking_dispatch(&mut instance)
            .map_err(|_| Error::Dispatch)?;

        instance.qh = Some(qh);
        instance.ev = Some(event_queue);
        Ok(instance)
    }

    pub fn next_frame(&mut self) -> Result<YUVBuffer, Error> {
        let mut ev = self.ev.take().unwrap();

        let frame = self
            .screencopy_mgr
            .as_ref()
            .ok_or(Error::ScreenManager)?
            .capture_output(
                self.show_cursor as i32,
                self.output.as_ref().ok_or(Error::Output)?,
                self.qh.as_ref().unwrap(),
                (),
            );
        ev.blocking_dispatch(self).map_err(|_| Error::Dispatch)?;

        frame.copy(self.buffer.as_ref().ok_or(Error::Buffer)?);
        ev.blocking_dispatch(self).map_err(|_| Error::Dispatch)?;
        frame.destroy();

        self.ev = Some(ev);
        self.ready_frame.take().ok_or(Error::NoFrame)
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WlBackend {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<WlBackend>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "zwlr_screencopy_manager_v1" => {
                    state.screencopy_mgr = Some(registry.bind::<ZwlrScreencopyManagerV1, _, _>(
                        name,
                        version.min(3),
                        qh,
                        (),
                    ));
                }

                "wl_shm" => {
                    state.shm = Some(registry.bind::<WlShm, _, _>(name, version, qh, ()));
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

impl Dispatch<ZwlrScreencopyFrameV1, ()> for WlBackend {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                let format = match format {
                    WEnum::Value(f) => f,
                    _ => unimplemented!("Unsupported pixel format for now"),
                };

                if state.width == width
                    && state.height == height
                    && state.stride == stride
                    && state.format == format
                {
                    return;
                }

                state.rgb_buffer = Vec::with_capacity((state.width * state.height * 3) as usize);
                state.width = width;
                state.height = height;
                state.stride = stride;
                state.format = format;

                assert_eq!(
                    state.format,
                    wl_shm::Format::Xrgb8888,
                    "Unsupported pixel format for now",
                );

                let size = state.stride * state.height;

                let fd = memfd::MemfdOptions::new()
                    .create("wl_frame_cap_shm")
                    .unwrap();

                fd.as_file().set_len(size as u64).unwrap();

                //SAFETY: we are reading mmap backed buffer only after frame is ready
                state.mmap = Some(unsafe { memmap2::Mmap::map(fd.as_file()).unwrap() });

                let pool: WlShmPool = state.shm.as_ref().unwrap().create_pool(
                    fd.as_file().as_fd(),
                    size as i32,
                    qhandle,
                    (),
                );

                state.buffer = Some(pool.create_buffer(
                    0,
                    state.width as i32,
                    state.height as i32,
                    state.stride as i32,
                    format,
                    qhandle,
                    (),
                ));
            }

            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                let buffer = state.mmap.as_ref().unwrap();
                state.rgb_buffer.clear();

                for pixels in buffer.chunks_exact(4) {
                    state
                        .rgb_buffer
                        .extend_from_slice(&[pixels[2], pixels[1], pixels[0]]);
                }

                state.ready_frame = Some(YUVBuffer::from_rgb8_source(RgbSliceU8::new(
                    &state.rgb_buffer,
                    (state.width as usize, state.height as usize),
                )));
            }

            _ => {}
        }
    }
}

impl Dispatch<WlBuffer, ()> for WlBackend {
    fn event(
        _state: &mut Self,
        _proxy: &WlBuffer,
        _event: <WlBuffer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShmPool, ()> for WlBackend {
    fn event(
        _state: &mut Self,
        _proxy: &WlShmPool,
        _event: <WlShmPool as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for WlBackend {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrScreencopyManagerV1,
        _event: <ZwlrScreencopyManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlOutput, ()> for WlBackend {
    fn event(
        _state: &mut Self,
        _proxy: &WlOutput,
        _event: <WlOutput as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShm, ()> for WlBackend {
    fn event(
        _state: &mut Self,
        _proxy: &WlShm,
        _event: <WlShm as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}
