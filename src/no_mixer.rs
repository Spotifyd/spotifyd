use librespot_playback::mixer::{Mixer, MixerConfig};

pub struct NoMixer;

impl Mixer for NoMixer {
    fn open(_: MixerConfig) -> NoMixer {
        NoMixer {}
    }

    fn volume(&self) -> u16 {
        u16::MAX
    }

    fn set_volume(&self, _volume: u16) {}
}
