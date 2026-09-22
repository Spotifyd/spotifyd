#![cfg(all(target_os = "macos", feature = "macos_media_remote"))]

use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicyAccessory};
use librespot_connect::Spirc;
use librespot_metadata::audio::UniqueFields;
use librespot_playback::player::PlayerEvent;
use log::{debug, error};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Registers this process as a real (backgrounded, dock-less) Cocoa
/// application. Without this, spotifyd is just a bare process making raw
/// `MPNowPlayingInfoCenter` calls -- it can win the Now Playing slot on a
/// cold start when nothing else is claiming it, but macOS's MediaRemote
/// arbitration won't let a non-app process reclaim the slot once it's lost
/// it to a real app (e.g. Music.app, another Spotify Connect client). This
/// must run once, before any other AppKit/MediaRemote call.
pub(crate) fn ensure_nsapplication() {
    unsafe {
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);
    }
}

/// Runs the Cocoa event loop on the calling thread, forever. Must be called
/// from the real OS main thread, after `ensure_nsapplication`. This is what
/// actually keeps spotifyd's Now Playing claim reclaimable -- without a live
/// run loop behind it, `MPNowPlayingInfoCenter` registration is a dead claim
/// macOS won't hand back once another app takes it. All of spotifyd's actual
/// work (tokio runtime, Spotify Connect, playback) runs on a separate thread
/// spawned before this is called; this thread never returns.
pub(crate) fn run_event_loop() -> ! {
    unsafe {
        let app = NSApp();
        app.run();
    }
    unreachable!("NSApplication::run never returns")
}

/// Bridges spotifyd's playback state directly into macOS's native Now Playing
/// system (MPNowPlayingInfoCenter / MPRemoteCommandCenter), so spotifyd shows
/// up in Control Center, the lock screen, and responds to hardware media keys
/// the same way a native app (Spotify, Music) would. This is the macOS
/// counterpart to `dbus_mpris`, driven by the same `PlayerEvent` stream.
pub(crate) struct MacosMediaRemote {
    controls: MediaControls,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<Duration>,
    cover_url: Option<String>,
    /// Last known playback state, used by `tick` to keep re-asserting our
    /// Now Playing claim between `PlayerEvent`s -- macOS evicts a claim it
    /// hasn't seen refreshed in a while (playing *or* paused) and falls back
    /// to launching Music.app, so a real player must ping this periodically
    /// regardless of transport state. `None` once genuinely stopped, since
    /// there's no active session to keep alive at that point.
    last_playback: Option<PlaybackSnapshot>,
}

#[derive(Clone, Copy)]
struct PlaybackSnapshot {
    is_playing: bool,
    /// Track position as of `captured_at`. For a playing track this is
    /// extrapolated forward by elapsed wall-clock time in `tick`; for a
    /// paused track it's used as-is, since position doesn't advance.
    position: Duration,
    captured_at: Instant,
}

impl MacosMediaRemote {
    pub fn new() -> Result<Self, souvlaki::Error> {
        let controls = MediaControls::new(PlatformConfig {
            display_name: "spotifyd",
            dbus_name: "spotifyd",
            hwnd: None,
        })?;
        Ok(Self {
            controls,
            title: None,
            artist: None,
            album: None,
            duration: None,
            cover_url: None,
            last_playback: None,
        })
    }

    fn attach_handler(&mut self, spirc: Arc<Spirc>) {
        let handler_result = self.controls.attach(move |event: MediaControlEvent| {
            debug!("macos media remote: received command {event:?}");
            let result = match event {
                MediaControlEvent::Play => spirc.play(),
                MediaControlEvent::Pause => spirc.pause(),
                MediaControlEvent::Toggle => spirc.play_pause(),
                MediaControlEvent::Next => spirc.next(),
                MediaControlEvent::Previous => spirc.prev(),
                MediaControlEvent::SetPosition(MediaPosition(position)) => {
                    spirc.set_position_ms(position.as_millis() as u32)
                }
                _ => Ok(()),
            };
            if let Err(err) = result {
                error!("macos media remote: command failed: {err}");
            }
        });
        if let Err(err) = handler_result {
            error!("macos media remote: failed to attach command handlers: {err:?}");
        }
    }

    pub fn set_session(&mut self, spirc: Arc<Spirc>) {
        self.attach_handler(spirc);
    }

    pub fn drop_session(&mut self) {
        self.last_playback = None;
        let _ = self.controls.detach();
        let _ = self.controls.set_playback(MediaPlayback::Stopped);
    }

    fn push_metadata(&mut self) {
        let metadata = MediaMetadata {
            title: self.title.as_deref(),
            artist: self.artist.as_deref(),
            album: self.album.as_deref(),
            cover_url: self.cover_url.as_deref(),
            duration: self.duration,
        };
        if let Err(err) = self.controls.set_metadata(metadata) {
            error!("macos media remote: failed to set metadata: {err:?}");
        }
    }

    fn push_playback(&mut self, playback: MediaPlayback) {
        debug!("macos media remote: push_playback {playback:?}");
        if let Err(err) = self.controls.set_playback(playback) {
            error!("macos media remote: failed to set playback state: {err:?}");
        }
    }

    /// Re-asserts the current playback state on a timer so macOS doesn't
    /// consider our Now Playing claim stale and evict it, whether we're
    /// playing or paused. No-op once genuinely stopped.
    pub fn tick(&mut self) {
        debug!(
            "macos media remote: tick (last_playback is_some={})",
            self.last_playback.is_some()
        );
        let Some(snapshot) = self.last_playback else {
            return;
        };
        if snapshot.is_playing {
            let position = snapshot.position + snapshot.captured_at.elapsed();
            self.push_playback(MediaPlayback::Playing {
                progress: Some(MediaPosition(position)),
            });
        } else {
            self.push_playback(MediaPlayback::Paused {
                progress: Some(MediaPosition(snapshot.position)),
            });
        }
    }

    pub fn handle_event(&mut self, event: &PlayerEvent) {
        debug!("macos media remote: handle_event {event:?}");
        match event {
            PlayerEvent::TrackChanged { audio_item } => {
                self.title = Some(audio_item.name.clone());
                self.duration = Some(Duration::from_millis(audio_item.duration_ms as u64));
                self.cover_url = audio_item
                    .covers
                    .iter()
                    .max_by_key(|cover| cover.width)
                    .map(|cover| cover.url.clone());

                let (artist, album) = match &audio_item.unique_fields {
                    UniqueFields::Track { artists, album, .. } => (
                        Some(
                            artists
                                .iter()
                                .map(|a| a.name.clone())
                                .collect::<Vec<_>>()
                                .join(", "),
                        ),
                        Some(album.clone()),
                    ),
                    UniqueFields::Local { artists, album, .. } => (artists.clone(), album.clone()),
                    UniqueFields::Episode { show_name, .. } => (Some(show_name.clone()), None),
                };
                self.artist = artist;
                self.album = album;

                self.push_metadata();
            }
            PlayerEvent::Playing { position_ms, .. } => {
                let position = Duration::from_millis(*position_ms as u64);
                self.last_playback = Some(PlaybackSnapshot {
                    is_playing: true,
                    position,
                    captured_at: Instant::now(),
                });
                self.push_playback(MediaPlayback::Playing {
                    progress: Some(MediaPosition(position)),
                });
            }
            PlayerEvent::Paused { position_ms, .. } => {
                let position = Duration::from_millis(*position_ms as u64);
                self.last_playback = Some(PlaybackSnapshot {
                    is_playing: false,
                    position,
                    captured_at: Instant::now(),
                });
                self.push_playback(MediaPlayback::Paused {
                    progress: Some(MediaPosition(position)),
                });
            }
            PlayerEvent::Stopped { .. } => {
                self.title = None;
                self.artist = None;
                self.album = None;
                self.duration = None;
                self.cover_url = None;
                self.last_playback = None;
                self.push_playback(MediaPlayback::Stopped);
            }
            PlayerEvent::Seeked { position_ms, .. }
            | PlayerEvent::PositionCorrection { position_ms, .. } => {
                let position = Duration::from_millis(*position_ms as u64);
                let is_playing = self.last_playback.is_none_or(|s| s.is_playing);
                self.last_playback = Some(PlaybackSnapshot {
                    is_playing,
                    position,
                    captured_at: Instant::now(),
                });
                self.push_playback(if is_playing {
                    MediaPlayback::Playing {
                        progress: Some(MediaPosition(position)),
                    }
                } else {
                    MediaPlayback::Paused {
                        progress: Some(MediaPosition(position)),
                    }
                });
            }
            _ => {}
        }
    }
}
