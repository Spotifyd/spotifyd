use librespot_playback::mixer::{AudioFilter, Mixer, MixerConfig};

pub struct NoMixer {}

impl Mixer for NoMixer {
    fn open(_: Option<MixerConfig>) -> NoMixer {
        NoMixer {}
    }

    fn start(&self) {}

    fn stop(&self) {}

    fn volume(&self) -> u16 {
        u16::MAX
    }

    fn set_volume(&self, _volume: u16) {}

    fn get_audio_filter(&self) -> Option<Box<dyn AudioFilter + Send>> {
        None
    }
}
