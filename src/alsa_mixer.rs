use std::error::Error;
use librespot::playback::mixer::{AudioFilter, Mixer};
use alsa;

pub struct AlsaMixer {
    pub device: String,
    pub mixer: String,
}

impl AlsaMixer {
    fn set_volume_with_err(&self, volume: u16) -> Result<(), Box<Error>> {
        let mixer = alsa::mixer::Mixer::new(&self.device, false)?;

        let selem_id = alsa::mixer::SelemId::new(&*self.mixer, 0);
        let elem = mixer.find_selem(&selem_id).ok_or("Couldn't find selem.")?;

        let (min, max) = elem.get_playback_volume_range();

        let volume_steps = (max - min) as f64;
        let normalised_volume = ((volume as f64).log(65535.0) * volume_steps).floor() as i64 + min;

        error!(
            "volume={},min={},normalized={}",
            volume, min, normalised_volume
        );
        elem.set_playback_volume_all(normalised_volume)?;
        Ok(())
    }
}

impl Mixer for AlsaMixer {
    fn open() -> AlsaMixer {
        AlsaMixer {
            device: "default".to_string(),
            mixer: "Master".to_string(),
        }
    }
    fn start(&self) {}
    fn stop(&self) {}

    fn volume(&self) -> u16 {
        let selem_id = alsa::mixer::SelemId::new(&*self.mixer, 0);
        match alsa::mixer::Mixer::new(&self.device, false)
            .ok()
            .as_ref()
            .and_then(|ref mixer| mixer.find_selem(&selem_id))
            .and_then(|elem| {
                let (min, max) = elem.get_playback_volume_range();
                elem.get_playback_volume(alsa::mixer::SelemChannelId::mono())
                    .ok()
                    .map(|volume| {
                        let volume_steps = max - min + 1;
                        ((volume - min) * (0xFFFF / volume_steps)) as u16
                    })
            }) {
            Some(vol) => vol,
            _ => {
                error!(
                    "Couldn't read volume from alsa device with name \"{}\".",
                    self.device
                );
                0
            }
        }
    }

    fn set_volume(&self, volume: u16) {
        match self.set_volume_with_err(volume) {
            Ok(_) => (),
            Err(e) => error!("Couldn't set volume: {:?}", e),
        }
    }

    fn get_audio_filter(&self) -> Option<Box<AudioFilter + Send>> {
        None
    }
}
