#![cfg(all(target_os = "macos", feature = "macos_media_remote"))]

use librespot_connect::Spirc;
use librespot_metadata::audio::UniqueFields;
use librespot_playback::player::PlayerEvent;
use log::{debug, error};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use std::sync::Arc;
use std::time::Duration;

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
        })
    }

    pub fn set_session(&mut self, spirc: Arc<Spirc>) {
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

    pub fn drop_session(&mut self) {
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

    pub fn handle_event(&mut self, event: &PlayerEvent) {
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
                self.push_playback(MediaPlayback::Playing {
                    progress: Some(MediaPosition(Duration::from_millis(*position_ms as u64))),
                });
            }
            PlayerEvent::Paused { position_ms, .. } => {
                self.push_playback(MediaPlayback::Paused {
                    progress: Some(MediaPosition(Duration::from_millis(*position_ms as u64))),
                });
            }
            PlayerEvent::Stopped { .. } => {
                self.title = None;
                self.artist = None;
                self.album = None;
                self.duration = None;
                self.cover_url = None;
                self.push_playback(MediaPlayback::Stopped);
            }
            PlayerEvent::Seeked { position_ms, .. }
            | PlayerEvent::PositionCorrection { position_ms, .. } => {
                self.push_playback(MediaPlayback::Playing {
                    progress: Some(MediaPosition(Duration::from_millis(*position_ms as u64))),
                });
            }
            _ => {}
        }
    }
}
