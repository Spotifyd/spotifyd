use librespot_playback::mixer::{Mixer, MixerConfig};

pub struct NoMixer;

impl Mixer for NoMixer {
    fn open(_: MixerConfig) -> Result<NoMixer, librespot_core::Error> {
        Ok(NoMixer {})
    }

    fn volume(&self) -> u16 {
        u16::MAX
    }

    fn set_volume(&self, _volume: u16) {}
}
