//! Native PipeWire audio backend for spotifyd.
//!
//! Talks to libpipewire directly via `pw::stream::Stream`, rather than
//! `--backend alsa --device pipewire` (ALSA's PipeWire PCM plugin). With no
//! `device`, PipeWire's session manager routes to the default sink; if
//! given, `device` is passed through as `node.target` to pin a specific
//! sink/node.
//!
//! The stream is tagged with a stable identity (`node.name` /
//! `application.name` = `"spotifyd"`), which `pipewire_mixer.rs`'s
//! "stream" mode relies on to find it in the graph.
//!
//! Written against pipewire-rs 0.8.0's "classic" API (`MainLoop`/
//! `Context`/`Core`, `context.connect(None)`, `core.get_registry()`).
//! Newer releases rename these to `MainLoopRc`/`ContextRc`/`connect_rc`/
//! `get_registry_rc` — swap the constructors if your lockfile resolves one
//! of those. The `process` callback's buffer accessors (`dequeue_buffer`,
//! `datas_mut`, `data()`, `chunk_mut()`) are the part most likely to need
//! adjustment on other point releases; the rest has been stable.
//!
//! `write()` converts samples by hand instead of using librespot's
//! `sink_as_bytes!()` macro or `Converter` — both are private to
//! `librespot_playback`, unreachable from an external crate. Integer
//! formats (S16/S24/S24_3/S32) get a plain clamp-and-round instead of
//! `Converter`'s dithering; F32/F64 are unaffected.
//!
//! `push_bytes()` (writer thread) and `process()` (realtime callback) are
//! coordinated via `SpaceSignal`, a Condvar the realtime side notifies
//! whenever it frees ring-buffer space, rather than a fixed poll interval —
//! the same approach MPD's PipeWire output uses
//! (`pw_thread_loop_wait`/`_signal`).

use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use color_eyre::eyre::{self, eyre};
use librespot_playback::{
    audio_backend::{Open, Sink, SinkError, SinkResult},
    config::AudioFormat,
    convert::Converter,
    decoder::AudioPacket,
};
use log::{error, info, warn};
use pipewire as pw;
use pw::{
    context::Context,
    core::Core,
    keys,
    main_loop::MainLoop,
    properties::properties,
    spa::{
        self,
        param::audio::{AudioFormat as SpaAudioFormat, AudioInfoRaw},
        pod::{serialize::PodSerializer, Pod, Value},
        utils::{Direction, SpaTypes},
    },
    stream::{Stream, StreamFlags},
};
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapRb,
};

/// Buffer between librespot's writer thread and PipeWire's realtime
/// `process` callback, in ms. Generous on purpose: costs a little latency
/// but starves less easily if the writer thread is briefly delayed. (MPD's
/// own PipeWire output uses 0.5s.)
const RING_BUFFER_MS: u64 = 2000;

const SAMPLE_RATE: u32 = librespot_playback::SAMPLE_RATE;
const NUM_CHANNELS: u16 = librespot_playback::NUM_CHANNELS as u16;

/// Commands sent from the public `Sink` handle into the background
/// PipeWire-loop thread.
enum LoopCommand {
    Shutdown,
}

/// Wakes the writer thread as soon as `process()` frees ring-buffer space,
/// instead of a fixed polling interval.
#[derive(Default)]
struct SpaceSignal {
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl SpaceSignal {
    fn notify(&self) {
        self.condvar.notify_one();
    }

    /// Blocks until notified or `timeout` elapses. The timeout only guards
    /// against a missed-wakeup race; the normal path is `notify()`.
    fn wait_timeout(&self, timeout: Duration) {
        let guard = self.mutex.lock().expect("lock shouldn't be poisoned");
        let _ = self.condvar.wait_timeout(guard, timeout);
    }
}

pub struct PipewireSink {
    format: AudioFormat,
    bytes_per_frame: usize,
    producer: Option<ringbuf::HeapProd<u8>>,
    space_signal: Option<Arc<SpaceSignal>>,
    /// Set while `process()` should expect data — cleared in `stop()` so
    /// idle calls between tracks don't log spurious underruns.
    is_active: Arc<AtomicBool>,
    pw_sender: Option<pw::channel::Sender<LoopCommand>>,
    thread_handle: Option<JoinHandle<()>>,
    device: Option<String>,
}

/// Matches `librespot_playback::audio_backend::SinkBuilder` exactly, so it
/// coerces directly in setup.rs — `PipewireSink` isn't in librespot's own
/// `BACKENDS` table, so `backend = "pipewire"` is handled as a separate
/// branch there instead of via `audio_backend::find`.
pub fn open(device: Option<String>, format: AudioFormat) -> Box<dyn Sink> {
    Box::new(PipewireSink::open(device, format))
}

impl Open for PipewireSink {
    fn open(device: Option<String>, format: AudioFormat) -> Self {
        info!("Using PipeWire sink with format: {format:?}");

        let bytes_per_sample: usize = match format {
            AudioFormat::F64 => 8,
            AudioFormat::F32 => 4,
            AudioFormat::S32 => 4,
            AudioFormat::S24 => 4,
            AudioFormat::S24_3 => 3,
            AudioFormat::S16 => 2,
        };
        let bytes_per_frame = bytes_per_sample * NUM_CHANNELS as usize;

        Self {
            format,
            bytes_per_frame,
            producer: None,
            space_signal: None,
            is_active: Arc::new(AtomicBool::new(false)),
            pw_sender: None,
            thread_handle: None,
            device,
        }
    }
}

impl Sink for PipewireSink {
    fn start(&mut self) -> SinkResult<()> {
        // Set unconditionally: start() can be called again for a new track
        // while a previous thread is still running (see below), and each
        // call needs to re-arm the underrun warning.
        self.is_active.store(true, Ordering::Relaxed);

        if self.thread_handle.is_some() {
            // Already running; PipeWire streams tolerate gaps in `process`
            // fine, so nothing is torn down between tracks.
            return Ok(());
        }

        let capacity = self.bytes_per_frame
            * (SAMPLE_RATE as usize * RING_BUFFER_MS as usize / 1000);
        let ring = HeapRb::<u8>::new(capacity.max(self.bytes_per_frame));
        let (producer, consumer) = ring.split();

        let space_signal = Arc::new(SpaceSignal::default());

        let (pw_sender, pw_receiver) = pw::channel::channel::<LoopCommand>();

        let format = self.format;
        let device = self.device.clone();
        let bytes_per_frame = self.bytes_per_frame;
        let process_signal = space_signal.clone();
        let process_is_active = self.is_active.clone();

        let thread_handle = thread::Builder::new()
            .name("pipewire-sink".into())
            .spawn(move || {
                if let Err(err) = run_pipewire_loop(
                    consumer,
                    pw_receiver,
                    format,
                    device,
                    bytes_per_frame,
                    process_signal,
                    process_is_active,
                ) {
                    error!("PipeWire sink loop exited with an error: {err:?}");
                }
            })
            .map_err(|e| SinkError::StateChange(format!("failed to spawn PipeWire thread: {e}")))?;

        self.producer = Some(producer);
        self.space_signal = Some(space_signal);
        self.pw_sender = Some(pw_sender);
        self.thread_handle = Some(thread_handle);

        Ok(())
    }

    fn stop(&mut self) -> SinkResult<()> {
        // Thread/stream stay alive (see start()); this just tells
        // process() an empty ring buffer is now expected, not an underrun.
        self.is_active.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn write(&mut self, packet: AudioPacket, _converter: &mut Converter) -> SinkResult<()> {
        let samples = packet
            .samples()
            .map_err(|e| SinkError::OnWrite(e.to_string()))?;

        let bytes_per_sample = self.bytes_per_frame / NUM_CHANNELS as usize;
        let mut bytes = Vec::with_capacity(samples.len() * bytes_per_sample);

        match self.format {
            AudioFormat::F64 => {
                for &s in samples {
                    bytes.extend_from_slice(&s.to_le_bytes());
                }
            }
            AudioFormat::F32 => {
                for &s in samples {
                    bytes.extend_from_slice(&(s as f32).to_le_bytes());
                }
            }
            AudioFormat::S32 => {
                for &s in samples {
                    let v = (s.clamp(-1.0, 1.0) * i32::MAX as f64).round() as i32;
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
            AudioFormat::S24 => {
                // 24-bit value in a 4-byte container (low 24 bits).
                for &s in samples {
                    let v = (s.clamp(-1.0, 1.0) * 8_388_607.0_f64).round() as i32;
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
            AudioFormat::S24_3 => {
                // 24-bit value packed into exactly 3 bytes, no padding.
                for &s in samples {
                    let v = (s.clamp(-1.0, 1.0) * 8_388_607.0_f64).round() as i32;
                    let le = v.to_le_bytes();
                    bytes.extend_from_slice(&le[..3]);
                }
            }
            AudioFormat::S16 => {
                for &s in samples {
                    let v = (s.clamp(-1.0, 1.0) * i16::MAX as f64).round() as i16;
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
        }

        self.push_bytes(&bytes)
    }
}

impl PipewireSink {
    fn push_bytes(&mut self, data: &[u8]) -> SinkResult<()> {
        let producer = self
            .producer
            .as_mut()
            .ok_or_else(|| SinkError::NotConnected("PipeWire sink not started".into()))?;
        let space_signal = self.space_signal.as_ref();

        // Blocking write, mirroring the ALSA/PulseAudio backends: push what
        // fits, wait for process() to free space (woken via SpaceSignal),
        // repeat until everything is written.
        let mut remaining = data;
        while !remaining.is_empty() {
            let written = producer.push_slice(remaining);
            remaining = &remaining[written..];
            if !remaining.is_empty() {
                match space_signal {
                    Some(sig) => sig.wait_timeout(Duration::from_millis(20)),
                    None => thread::sleep(Duration::from_millis(5)),
                }
            }
        }

        Ok(())
    }
}

impl Drop for PipewireSink {
    fn drop(&mut self) {
        if let Some(sender) = self.pw_sender.take() {
            let _ = sender.send(LoopCommand::Shutdown);
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

fn spa_format_for(format: AudioFormat) -> SpaAudioFormat {
    match format {
        AudioFormat::F64 => SpaAudioFormat::F64LE,
        AudioFormat::F32 => SpaAudioFormat::F32LE,
        AudioFormat::S32 => SpaAudioFormat::S32LE,
        // No native "24-in-32" SPA format distinct from S32; the 4-byte
        // S24 container maps to S24_32LE.
        AudioFormat::S24 => SpaAudioFormat::S24_32LE,
        AudioFormat::S24_3 => SpaAudioFormat::S24LE,
        AudioFormat::S16 => SpaAudioFormat::S16LE,
    }
}

/// Runs entirely on the dedicated PipeWire thread: owns the mainloop,
/// stream, and ring-buffer consumer for the sink's lifetime.
fn run_pipewire_loop(
    mut consumer: ringbuf::HeapCons<u8>,
    pw_receiver: pw::channel::Receiver<LoopCommand>,
    format: AudioFormat,
    device: Option<String>,
    bytes_per_frame: usize,
    space_signal: Arc<SpaceSignal>,
    is_active: Arc<AtomicBool>,
) -> eyre::Result<()> {
    pw::init();

    let mainloop = MainLoop::new(None)?;
    let context = Context::new(&mainloop)?;
    let core: Core = context.connect(None)?;

    let mut props = properties! {
        *keys::MEDIA_TYPE => "Audio",
        *keys::MEDIA_CATEGORY => "Playback",
        *keys::MEDIA_ROLE => "Music",
        // Stable identity, also relied on by pipewire_mixer.rs's "stream" mode.
        *keys::NODE_NAME => "spotifyd",
        *keys::APP_NAME => "spotifyd",
        *keys::NODE_DESCRIPTION => "spotifyd",
    };
    if let Some(target) = device.as_deref().filter(|d| !d.is_empty()) {
        // Raw key, not a `keys::` constant: `target.object` isn't in this
        // pipewire-rs version's `keys` module; `node.target` is the
        // long-standing equivalent.
        props.insert("node.target", target);
    }

    let stream = Stream::new(&core, "spotifyd", props)?;

    let mut audio_info = AudioInfoRaw::new();
    audio_info.set_format(spa_format_for(format));
    audio_info.set_rate(SAMPLE_RATE);
    audio_info.set_channels(NUM_CHANNELS as u32);

    let pod_bytes = PodSerializer::serialize(
        Cursor::new(Vec::new()),
        &Value::Object(spa::pod::Object {
            type_: SpaTypes::ObjectParamFormat.as_raw(),
            id: spa::param::ParamType::EnumFormat.as_raw(),
            properties: audio_info.into(),
        }),
    )
    .map_err(|_| eyre!("failed to serialize PipeWire audio format pod"))?
    .0
    .into_inner();

    let pod = Pod::from_bytes(&pod_bytes).ok_or_else(|| eyre!("failed to build format Pod"))?;
    let mut params = [pod];

    let _listener = stream
        .add_local_listener_with_user_data(())
        .state_changed(|_, _, old, new| {
            info!("PipeWire stream state: {old:?} -> {new:?}");
        })
        .process(move |stream, _| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                warn!("PipeWire: no buffer available in process callback");
                return;
            };
            let datas = buffer.datas_mut();
            let Some(data) = datas.get_mut(0) else { return };
            let Some(slice) = data.data() else { return };

            let capacity = slice.len();
            let usable = capacity - (capacity % bytes_per_frame);
            let available = consumer.occupied_len().min(usable);

            let popped = consumer.pop_slice(&mut slice[..available]);
            if popped < usable && is_active.load(Ordering::Relaxed) {
                // Gated on is_active: an empty buffer between tracks is
                // expected (stop() doesn't tear the stream down), so only
                // log while we're actually supposed to be playing.
                warn!(
                    "PipeWire: ring buffer underrun, padded {} of {} bytes",
                    usable - popped,
                    capacity
                );
            }
            for byte in &mut slice[popped..capacity] {
                *byte = 0; // silence-pad on underrun
            }

            let chunk = data.chunk_mut();
            *chunk.offset_mut() = 0;
            *chunk.stride_mut() = bytes_per_frame as _;
            *chunk.size_mut() = capacity as _;

            if popped > 0 {
                space_signal.notify();
            }
        })
        .register()?;

    stream.connect(
        Direction::Output,
        None,
        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;

    let _receiver = pw_receiver.attach(mainloop.loop_(), {
        let mainloop = mainloop.clone();
        move |cmd| match cmd {
            LoopCommand::Shutdown => mainloop.quit(),
        }
    });

    mainloop.run();

    Ok(())
}
