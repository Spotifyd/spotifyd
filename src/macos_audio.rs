//! macOS default audio output tracking.
//!
//! `librespot-playback` backends such as PortAudio query CoreAudio's device list and default
//! output device once when the sink is constructed (`Pa_Initialize` + `get_default_output_index`
//! in `PortAudioSink::open`). On macOS, connecting or disconnecting Bluetooth headphones,
//! plugging into a dock, or switching outputs in Control Center changes CoreAudio's
//! `kAudioHardwarePropertyDefaultOutputDevice` without notifying PortAudio, leaving a long-running
//! `spotifyd` writing to the stale device.
//!
//! When no explicit `--device` is configured (`device.is_none()`), [`open_sink`] wraps the
//! backend in [`DefaultOutputSink`], which
//!   * queries `kAudioHardwarePropertyDefaultOutputDevice` from `CoreAudio.framework` and
//!     reopens the underlying sink mid-stream when the default output `AudioDeviceID` changes;
//!   * tears the inner sink down on `stop()` so no CoreAudio stream is held open while idle and
//!     every `start()` re-initialises the backend against the current default output device.

use librespot_playback::{
    audio_backend::{Sink, SinkBuilder, SinkResult},
    config::AudioFormat,
    convert::Converter,
    decoder::AudioPacket,
};
use log::info;

#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyData(
        object_id: u32,
        address: *const AudioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const std::ffi::c_void,
        data_size: *mut u32,
        data: *mut std::ffi::c_void,
    ) -> i32;
}

/// CoreAudio's current default output `AudioDeviceID` (`kAudioHardwarePropertyDefaultOutputDevice`).
fn default_output_device_id() -> Option<u32> {
    const SYSTEM_OBJECT: u32 = 1; // kAudioObjectSystemObject
    const DEFAULT_OUTPUT_DEVICE: u32 = u32::from_be_bytes(*b"dOut"); // kAudioHardwarePropertyDefaultOutputDevice
    const SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob"); // kAudioObjectPropertyScopeGlobal
    const ELEMENT_MAIN: u32 = 0; // kAudioObjectPropertyElementMain

    let address = AudioObjectPropertyAddress {
        selector: DEFAULT_OUTPUT_DEVICE,
        scope: SCOPE_GLOBAL,
        element: ELEMENT_MAIN,
    };
    let mut device_id: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM_OBJECT,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            (&mut device_id as *mut u32).cast(),
        )
    };
    (status == 0 && device_id != 0).then_some(device_id)
}

pub(crate) fn open_sink(
    backend: SinkBuilder,
    device: Option<String>,
    format: AudioFormat,
) -> Box<dyn Sink> {
    if device.is_none() {
        Box::new(DefaultOutputSink::new(backend, format))
    } else {
        backend(device, format)
    }
}

struct DefaultOutputSink {
    backend: SinkBuilder,
    format: AudioFormat,
    inner: Option<Box<dyn Sink>>,
    device_id: Option<u32>,
}

impl DefaultOutputSink {
    fn new(backend: SinkBuilder, format: AudioFormat) -> Self {
        Self {
            backend,
            format,
            inner: None,
            device_id: None,
        }
    }

    fn close_inner(&mut self) -> SinkResult<()> {
        self.device_id = None;
        let Some(mut sink) = self.inner.take() else {
            return Ok(());
        };
        // Stop before drop so PortAudioSink closes its Stream before Pa_Terminate().
        let res = sink.stop();
        drop(sink);
        res
    }

    fn ensure_started(&mut self) -> SinkResult<&mut dyn Sink> {
        let current = default_output_device_id();
        if self.inner.is_none() || (current.is_some() && current != self.device_id) {
            if self.inner.is_some() {
                info!("default audio output device changed; reopening audio sink");
            }
            let _ = self.close_inner();
            let mut sink = (self.backend)(None, self.format);
            sink.start()?;
            self.device_id = current;
            self.inner = Some(sink);
        }
        Ok(self.inner.as_deref_mut().expect("sink initialised"))
    }
}

impl Sink for DefaultOutputSink {
    fn start(&mut self) -> SinkResult<()> {
        self.ensure_started().map(|_| ())
    }

    fn stop(&mut self) -> SinkResult<()> {
        self.close_inner()
    }

    fn write(&mut self, packet: AudioPacket, converter: &mut Converter) -> SinkResult<()> {
        self.ensure_started()?.write(packet, converter)
    }
}

impl Drop for DefaultOutputSink {
    fn drop(&mut self) {
        let _ = self.close_inner();
    }
}
