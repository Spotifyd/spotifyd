//! PipeWire volume control for spotifyd.
//!
//! `mixer = "sink" | "stream"` (via `MixerConfig.control`) selects the mode:
//!   * `"sink"` (default) — controls the system sink, resolved by name
//!     (`MixerConfig.device`) or via PipeWire's `default.audio.sink`
//!     metadata. Also pins spotifyd's own stream to unity gain so the
//!     sink-level change isn't doubled by a remembered per-stream volume.
//!     For a hardware sink backed by a Device Route (UCM/ALSA-routed
//!     outputs), the Route is written instead of the sink Node's own Props,
//!     which is a separate software gain layered on top of the graph.
//!   * `"stream"` — controls only spotifyd's own playback stream.
//!
//! Both modes require `backend = "pipewire"` (see `pipewire_backend.rs`) to
//! tag the stream's `node.name` as `"spotifyd"`.
//!
//! Runs its own main loop on a dedicated thread (PipeWire objects aren't
//! `Send`/`Sync`). Every sink/device/own-stream node is bound once at
//! discovery and its proxy kept alive for as long as it exists in the
//! graph, rather than rebinding by id later.
//!
//! Identifies as `media.category = "Manager"` (as `wpctl` does), since
//! WirePlumber's default access policy can otherwise block writes to
//! another object's (e.g. a Device's) parameters.
//!
//! `volume()` returns a local cache of the last value set, not a live
//! query — same limitation as upstream `AlsaMixer` (librespot#1450).
//!
//! Written against the "classic" `MainLoop`/`Context`/`Core` API on
//! pipewire-rs 0.8.0.
//!
//! ## Volume curve
//! `volume_ctrl = "pipewire"` (log) vs `"pipewire_linear"` in
//! spotifyd.conf selects how the raw Spotify Connect value maps to `norm`
//! (0.0-1.0), independently of PipeWire's own channelVolumes convention,
//! where the stored value is linear amplitude and `pct = cbrt(stored)` /
//! `stored = pct³` relates it to the human-facing percentage:
//!   * Linear — `norm` is a plain fraction of raw volume; `powf(3.0)` is
//!     applied once, in `set_props`/`set_route_volume`, to convert it to
//!     stored amplitude.
//!   * Log — `norm` is already a full curve (bounded dB taper, see
//!     `TAPER_MIN_DB`) and is written straight through with no further
//!     curve, to avoid double-curving.
//!
//! `apply_cubic` (set once in `open`, threaded down to
//! `set_props`/`set_route_volume`) is `true` only for `VolumeCtrl::Linear`.

use std::{
    cell::RefCell,
    collections::HashMap,
    io::Cursor,
    rc::Rc,
    sync::{
        mpsc::{self, RecvTimeoutError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use color_eyre::eyre::eyre;
use librespot_playback::mixer::{Mixer, MixerConfig};
use log::{error, info, warn};
use pipewire as pw;
use pw::{
    context::Context,
    core::Core,
    device::Device,
    main_loop::MainLoop,
    node::Node,
    properties::properties,
    registry::GlobalObject,
    spa::{
        self,
        param::ParamType,
        pod::{
            deserialize::PodDeserializer, serialize::PodSerializer, Object, Pod, Property,
            PropertyFlags, Value, ValueArray,
        },
        utils::dict::DictRef,
    },
    types::ObjectType,
};

/// Must match `pipewire_backend.rs`'s stream name/app-name properties.
const OWN_STREAM_NAME: &str = "spotifyd";

const METADATA_DEFAULT: &str = "default";
const DEFAULT_SINK_KEY: &str = "default.audio.sink";

/// dB range for the log-curve taper in `Mixer::set_volume`/`volume`. See
/// the module-level "Volume curve" doc comment.
const TAPER_MIN_DB: f64 = -50.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Sink,
    Stream,
}

enum MixerCommand {
    SetVolume(f64),
    Shutdown,
}

pub struct PipewireMixer {
    config: MixerConfig,
    volume_norm: Arc<Mutex<f64>>,
    sender: pw::channel::Sender<MixerCommand>,
    thread_handle: Option<JoinHandle<()>>,
}

impl PipewireMixer {
    fn set_volume_norm(&self, norm: f64) {
        let norm = norm.clamp(0.0, 1.0);
        *self.volume_norm.lock().expect("lock shouldn't be poisoned") = norm;
        if self.sender.send(MixerCommand::SetVolume(norm)).is_err() {
            error!("pipewire mixer: background thread is no longer running");
        }
    }

    fn volume_norm(&self) -> f64 {
        *self.volume_norm.lock().expect("lock shouldn't be poisoned")
    }
}

impl Mixer for PipewireMixer {
    fn open(config: MixerConfig) -> Result<PipewireMixer, librespot_core::Error> {
        let target = match config.control.to_lowercase().as_str() {
            "sink" | "pwsink" => Target::Sink,
            _ => Target::Stream,
        };
        let device_override = (!config.device.is_empty()).then(|| config.device.clone());
        let apply_cubic = matches!(
            config.volume_ctrl,
            librespot_playback::config::VolumeCtrl::Linear
        );

        info!(
            "Using PipeWire mixer in {target:?} mode ({} curve)",
            if apply_cubic { "linear" } else { "log" }
        );

        let (sender, receiver) = pw::channel::channel::<MixerCommand>();
        let (startup_tx, startup_rx) = mpsc::channel::<Result<(), String>>();

        let thread_handle = thread::Builder::new()
            .name("pipewire-mixer".into())
            .spawn(move || {
                run_mixer_loop(target, device_override, apply_cubic, receiver, startup_tx)
            })
            .map_err(|e| {
                librespot_core::Error::invalid_argument(eyre!(
                    "failed to spawn PipeWire mixer thread: {e}"
                ))
            })?;

        match startup_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => {}
            Ok(Err(msg)) => return Err(librespot_core::Error::invalid_argument(eyre!(msg))),
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
                return Err(librespot_core::Error::invalid_argument(eyre!(
                    "timed out connecting to PipeWire for volume control"
                )));
            }
        }

        Ok(PipewireMixer {
            config,
            volume_norm: Arc::new(Mutex::new(1.0)),
            sender,
            thread_handle: Some(thread_handle),
        })
    }

    fn volume(&self) -> u16 {
        let norm = self.volume_norm();
        let linear = match self.config.volume_ctrl {
            librespot_playback::config::VolumeCtrl::Linear => norm,
            _ => {
                if norm <= 0.0 {
                    0.0
                } else {
                    (1.0 - (20.0 * norm.log10()) / TAPER_MIN_DB).clamp(0.0, 1.0)
                }
            }
        };
        (linear * u16::MAX as f64).round() as u16
    }

    fn set_volume(&self, volume: u16) {
        let pos = volume as f64 / u16::MAX as f64;
        let norm = match self.config.volume_ctrl {
            librespot_playback::config::VolumeCtrl::Linear => pos,
            _ => {
                let db = TAPER_MIN_DB * (1.0 - pos);
                10f64.powf(db / 20.0)
            }
        };
        self.set_volume_norm(norm);
    }
}

impl Drop for PipewireMixer {
    fn drop(&mut self) {
        let _ = self.sender.send(MixerCommand::Shutdown);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// A bound `Audio/Sink` node.
struct SinkEntry {
    name: String,
    node: Node,
    /// Owning `Audio/Device` id (from `device.id`), if any.
    device_id: Option<u32>,
}

/// A bound `Audio/Device`, with its active output Route once known.
struct DeviceEntry {
    device: Device,
    /// `(index, device)` identity of the active output Route. `None` until
    /// the first Route param event arrives. Only the first Output-direction
    /// route seen is tracked; multi-route devices (e.g. speaker + headphone
    /// jack) aren't disambiguated.
    route: Option<(i32, i32)>,
    /// Whether `force_volume_resync` has already run once for this device.
    did_initial_resync: bool,
}

struct GraphState {
    sinks: HashMap<u32, SinkEntry>,
    devices: HashMap<u32, DeviceEntry>,
    own_stream: Option<(u32, Node)>,
    default_sink_name: Option<String>,
}

fn run_mixer_loop(
    target: Target,
    device_override: Option<String>,
    apply_cubic: bool,
    mixer_receiver: pw::channel::Receiver<MixerCommand>,
    startup_tx: mpsc::Sender<Result<(), String>>,
) {
    pw::init();

    let mainloop = match MainLoop::new(None) {
        Ok(m) => m,
        Err(e) => {
            let _ = startup_tx.send(Err(format!("failed to create PipeWire main loop: {e}")));
            return;
        }
    };
    let context = match Context::with_properties(
        &mainloop,
        properties! {
            *pw::keys::MEDIA_CATEGORY => "Manager",
            *pw::keys::APP_NAME => "spotifyd-pipewire-mixer",
        },
    ) {
        Ok(c) => c,
        Err(e) => {
            let _ = startup_tx.send(Err(format!("failed to create PipeWire context: {e}")));
            return;
        }
    };
    let core: Core = match context.connect(None) {
        Ok(c) => c,
        Err(e) => {
            let _ = startup_tx.send(Err(format!("failed to connect to PipeWire: {e}")));
            return;
        }
    };
    let registry = match core.get_registry() {
        Ok(r) => Rc::new(r),
        Err(e) => {
            let _ = startup_tx.send(Err(format!("failed to get PipeWire registry: {e}")));
            return;
        }
    };

    let _ = startup_tx.send(Ok(()));

    let state = Rc::new(RefCell::new(GraphState {
        sinks: HashMap::new(),
        devices: HashMap::new(),
        own_stream: None,
        default_sink_name: None,
    }));

    let metadata_holder: Rc<
        RefCell<Option<(pw::metadata::Metadata, pw::metadata::MetadataListener)>>,
    > = Rc::new(RefCell::new(None));

    let device_listeners: Rc<RefCell<HashMap<u32, pw::device::DeviceListener>>> =
        Rc::new(RefCell::new(HashMap::new()));

    let _registry_listener = registry
        .add_listener_local()
        .global({
            let registry = registry.clone();
            let state = state.clone();
            let metadata_holder = metadata_holder.clone();
            let device_listeners = device_listeners.clone();
            move |global| {
                handle_global(
                    &registry,
                    global,
                    &state,
                    &metadata_holder,
                    &device_listeners,
                )
            }
        })
        .global_remove({
            let state = state.clone();
            let device_listeners = device_listeners.clone();
            move |id| {
                let mut state = state.borrow_mut();
                state.sinks.remove(&id);
                state.devices.remove(&id);
                device_listeners.borrow_mut().remove(&id);
                if state.own_stream.as_ref().is_some_and(|(sid, _)| *sid == id) {
                    state.own_stream = None;
                }
            }
        })
        .register();

    let _receiver = mixer_receiver.attach(mainloop.loop_(), {
        let mainloop = mainloop.clone();
        let state = state.clone();
        move |cmd| match cmd {
            MixerCommand::SetVolume(norm) => {
                apply_volume(target, &device_override, apply_cubic, &state, norm)
            }
            MixerCommand::Shutdown => mainloop.quit(),
        }
    });

    mainloop.run();
}

fn handle_global(
    registry: &Rc<pw::registry::Registry>,
    global: &GlobalObject<&DictRef>,
    state: &Rc<RefCell<GraphState>>,
    metadata_holder: &Rc<RefCell<Option<(pw::metadata::Metadata, pw::metadata::MetadataListener)>>>,
    device_listeners: &Rc<RefCell<HashMap<u32, pw::device::DeviceListener>>>,
) {
    match global.type_ {
        ObjectType::Node => {
            let Some(props) = global.props else { return };
            let media_class = props.get("media.class").unwrap_or_default();
            let name = props
                .get("node.name")
                .or_else(|| props.get("node.description"))
                .unwrap_or_default()
                .to_string();
            let app_name = props.get("application.name").unwrap_or_default();

            if media_class == "Audio/Sink" {
                let device_id = props.get("device.id").and_then(|s| s.parse::<u32>().ok());
                match registry.bind::<Node, _>(global) {
                    Ok(node) => {
                        state.borrow_mut().sinks.insert(
                            global.id,
                            SinkEntry {
                                name,
                                node,
                                device_id,
                            },
                        );
                    }
                    Err(e) => warn!(
                        "pipewire mixer: failed to bind sink node {}: {e:?}",
                        global.id
                    ),
                }
            } else if media_class == "Stream/Output/Audio"
                && (name == OWN_STREAM_NAME || app_name == OWN_STREAM_NAME)
            {
                match registry.bind::<Node, _>(global) {
                    Ok(node) => {
                        state.borrow_mut().own_stream = Some((global.id, node));
                    }
                    Err(e) => warn!("pipewire mixer: failed to bind own stream node: {e:?}"),
                }
            }
        }
        ObjectType::Device => {
            let Some(props) = global.props else { return };
            if props.get("media.class") != Some("Audio/Device") {
                return;
            }
            match registry.bind::<Device, _>(global) {
                Ok(device) => {
                    let id = global.id;
                    let listener = device
                        .add_listener_local()
                        .param({
                            let state = state.clone();
                            move |_seq, param_id, _index, _next, param| {
                                if param_id != ParamType::Route {
                                    return;
                                }
                                let Some(param) = param else { return };
                                let Some(identity) = parse_route_identity(param) else {
                                    return;
                                };
                                if let Some(entry) = state.borrow_mut().devices.get_mut(&id) {
                                    if entry.route.is_none() {
                                        info!(
                                            "pipewire mixer: resolved route (index={}, device={}) \
                                             for device id {id}",
                                            identity.0, identity.1
                                        );
                                        entry.route = Some(identity);
                                    }
                                }
                            }
                        })
                        .register();
                    device.enum_params(0, Some(ParamType::Route), 0, u32::MAX);

                    state.borrow_mut().devices.insert(
                        id,
                        DeviceEntry {
                            device,
                            route: None,
                            did_initial_resync: false,
                        },
                    );
                    device_listeners.borrow_mut().insert(id, listener);
                }
                Err(e) => warn!("pipewire mixer: failed to bind device {}: {e:?}", global.id),
            }
        }
        ObjectType::Metadata => {
            let Some(props) = global.props else { return };
            if props.get("metadata.name") != Some(METADATA_DEFAULT) {
                return;
            }
            match registry.bind::<pw::metadata::Metadata, _>(global) {
                Ok(metadata) => {
                    let state = state.clone();
                    let listener = metadata
                        .add_listener_local()
                        .property(move |_subject, key, _type, value| {
                            if key == Some(DEFAULT_SINK_KEY) {
                                let resolved = value.and_then(|v| extract_json_name(v));
                                state.borrow_mut().default_sink_name = resolved;
                            }
                            0
                        })
                        .register();
                    *metadata_holder.borrow_mut() = Some((metadata, listener));
                }
                Err(e) => warn!("pipewire mixer: failed to bind default metadata object: {e:?}"),
            }
        }
        _ => {}
    }
}

fn apply_volume(
    target: Target,
    device_override: &Option<String>,
    apply_cubic: bool,
    state: &Rc<RefCell<GraphState>>,
    volume_norm: f64,
) {
    let mut state = state.borrow_mut();

    match target {
        Target::Sink => {
            let desired_name = device_override
                .clone()
                .or_else(|| state.default_sink_name.clone());

            match desired_name {
                Some(name) => {
                    let found = state
                        .sinks
                        .values()
                        .find(|s| s.name == name)
                        .map(|s| (s.device_id, ()));

                    match found {
                        Some((Some(device_id), ())) if state.devices.contains_key(&device_id) => {
                            let entry = state.devices.get_mut(&device_id).unwrap();
                            let route_written = match entry.route {
                                Some(route_id) => {
                                    if !entry.did_initial_resync {
                                        force_volume_resync(
                                            &entry.device,
                                            route_id,
                                            volume_norm,
                                            apply_cubic,
                                        );
                                        entry.did_initial_resync = true;
                                    } else {
                                        set_route_volume(
                                            &entry.device,
                                            route_id,
                                            volume_norm,
                                            apply_cubic,
                                        );
                                    }
                                    info!(
                                        "pipewire mixer: set route (index={}, device={}) on \
                                         '{name}' to {volume_norm:.3}",
                                        route_id.0, route_id.1
                                    );
                                    true
                                }
                                None => {
                                    warn!(
                                        "pipewire mixer: sink '{name}' has a device but no \
                                         Route resolved yet"
                                    );
                                    false
                                }
                            };
                            // Pin sink's own software gain to unity, but
                            // only once the hardware Route write above
                            // actually landed.
                            if route_written {
                                if let Some(sink) = state.sinks.values().find(|s| s.name == name) {
                                    set_props(&sink.node, 1.0, false, apply_cubic);
                                }
                            }
                        }
                        Some((_, ())) => {
                            if let Some(sink) = state.sinks.values().find(|s| s.name == name) {
                                set_props(&sink.node, volume_norm, false, apply_cubic);
                            }
                        }
                        None => warn!("pipewire mixer: sink '{name}' not present in the graph yet"),
                    }
                }
                None => warn!("pipewire mixer: no default sink resolved yet"),
            }

            if let Some((_, node)) = &state.own_stream {
                set_props(node, 1.0, false, apply_cubic);
            }
        }
        Target::Stream => match &state.own_stream {
            Some((_, node)) => set_props(node, volume_norm, false, apply_cubic),
            None => warn!(
                "pipewire mixer: spotifyd's own stream isn't in the graph yet \
                 (is `backend = \"pipewire\"` active and playing?)"
            ),
        },
    }
}

/// Writes a Node's own Props volume. For a Route-backed sink this is a
/// separate software gain layered on top of the graph, not the hardware
/// control — see `set_route_volume`.
fn set_props(node: &Node, volume_norm: f64, mute: bool, apply_cubic: bool) {
    let volume_norm = volume_norm.clamp(0.0, 1.0);
    let volume = if apply_cubic {
        volume_norm.powf(3.0)
    } else {
        volume_norm
    } as f32;

    let value = Value::Object(Object {
        type_: spa::utils::SpaTypes::ObjectParamProps.as_raw(),
        id: ParamType::Props.as_raw(),
        properties: vec![
            Property {
                key: spa::sys::SPA_PROP_mute,
                flags: PropertyFlags::empty(),
                value: Value::Bool(mute),
            },
            Property {
                key: spa::sys::SPA_PROP_channelVolumes,
                flags: PropertyFlags::empty(),
                value: Value::ValueArray(ValueArray::Float(vec![volume, volume])),
            },
        ],
    });

    let pod_bytes = match PodSerializer::serialize(Cursor::new(Vec::new()), &value) {
        Ok((cursor, _)) => cursor.into_inner(),
        Err(_) => {
            error!("pipewire mixer: failed to serialize Props pod");
            return;
        }
    };
    let Some(pod) = Pod::from_bytes(&pod_bytes) else {
        error!("pipewire mixer: failed to build Props pod");
        return;
    };
    node.set_param(ParamType::Props, 0, &pod);
}

/// Writes a Device's active output Route — the real hardware mixer control
/// for Route/UCM-backed sinks.
fn set_route_volume(device: &Device, route_id: (i32, i32), volume_norm: f64, apply_cubic: bool) {
    let (index, dev) = route_id;
    let volume_norm = volume_norm.clamp(0.0, 1.0);
    let volume = if apply_cubic {
        volume_norm.powf(3.0)
    } else {
        volume_norm
    } as f32;

    let props = Value::Object(Object {
        type_: spa::utils::SpaTypes::ObjectParamProps.as_raw(),
        id: ParamType::Route.as_raw(),
        properties: vec![Property {
            key: spa::sys::SPA_PROP_channelVolumes,
            flags: PropertyFlags::empty(),
            value: Value::ValueArray(ValueArray::Float(vec![volume, volume])),
        }],
    });

    let value = Value::Object(Object {
        type_: spa::utils::SpaTypes::ObjectParamRoute.as_raw(),
        id: ParamType::Route.as_raw(),
        properties: vec![
            Property {
                key: spa::sys::SPA_PARAM_ROUTE_index,
                flags: PropertyFlags::empty(),
                value: Value::Int(index),
            },
            Property {
                key: spa::sys::SPA_PARAM_ROUTE_device,
                flags: PropertyFlags::empty(),
                value: Value::Int(dev),
            },
            Property {
                key: spa::sys::SPA_PARAM_ROUTE_props,
                flags: PropertyFlags::empty(),
                value: props,
            },
            Property {
                key: spa::sys::SPA_PARAM_ROUTE_save,
                flags: PropertyFlags::empty(),
                value: Value::Bool(true),
            },
        ],
    });

    let pod_bytes = match PodSerializer::serialize(Cursor::new(Vec::new()), &value) {
        Ok((cursor, _)) => cursor.into_inner(),
        Err(_) => {
            error!("pipewire mixer: failed to serialize Route pod");
            return;
        }
    };
    let Some(pod) = Pod::from_bytes(&pod_bytes) else {
        error!("pipewire mixer: failed to build Route pod");
        return;
    };
    device.set_param(ParamType::Route, 0, &pod);
}

/// WirePlumber restores its own last-known Route volume at startup and may
/// skip the hardware write if the target already matches its cache. Nudge
/// off-target then back so the real write can't be a no-op. Runs once per
/// device (`DeviceEntry::did_initial_resync`).
fn force_volume_resync(device: &Device, route_id: (i32, i32), target_norm: f64, apply_cubic: bool) {
    let target_norm = target_norm.clamp(0.0, 1.0);
    let nudge = if target_norm < 0.5 {
        target_norm + 0.01
    } else {
        target_norm - 0.01
    };
    set_route_volume(device, route_id, nudge.clamp(0.0, 1.0), apply_cubic);
    set_route_volume(device, route_id, target_norm, apply_cubic);
}

/// `(index, device)` identity from a live `Spa:Enum:ParamId:Route` event.
/// Ignores anything that isn't `Spa:Enum:Direction:Output`.
fn parse_route_identity(pod: &Pod) -> Option<(i32, i32)> {
    let (_, value) = PodDeserializer::deserialize_from::<Value>(pod.as_bytes()).ok()?;
    let Value::Object(obj) = value else {
        return None;
    };

    let mut index = None;
    let mut device = None;
    let mut direction_is_output = false;

    for prop in &obj.properties {
        match prop.key {
            k if k == spa::sys::SPA_PARAM_ROUTE_index => {
                if let Value::Int(i) = prop.value {
                    index = Some(i);
                }
            }
            k if k == spa::sys::SPA_PARAM_ROUTE_device => {
                if let Value::Int(i) = prop.value {
                    device = Some(i);
                }
            }
            k if k == spa::sys::SPA_PARAM_ROUTE_direction => {
                if let Value::Id(id) = prop.value {
                    direction_is_output = id.0 == 1; // Spa:Enum:Direction:Output
                }
            }
            _ => {}
        }
    }

    if !direction_is_output {
        return None;
    }
    Some((index?, device?))
}

/// Minimal extractor for `{"name":"some.node.name"}` metadata values.
fn extract_json_name(json: &str) -> Option<String> {
    let key = "\"name\"";
    let start = json.find(key)? + key.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let after_colon = &rest[colon + 1..];
    let quote_start = after_colon.find('"')? + 1;
    let after_quote = &after_colon[quote_start..];
    let quote_end = after_quote.find('"')?;
    Some(after_quote[..quote_end].to_string())
}
