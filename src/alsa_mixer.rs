use librespot_playback::mixer::{Mixer, MixerConfig};
use log::error;
use std::error::Error;

#[derive(Clone)]
pub struct AlsaMixer {
    pub device: String,
    pub mixer: String,
    pub linear_scaling: bool,
}

impl AlsaMixer {
    fn set_volume_with_err(&self, volume: u16) -> Result<(), Box<dyn Error>> {
        let mixer = alsa::mixer::Mixer::new(&self.device, false)?;

        let selem_id = alsa::mixer::SelemId::new(&self.mixer, 0);
        let elem = mixer.find_selem(&selem_id).ok_or_else(|| {
            format!(
                "Couldn't find selem with name '{}'.",
                selem_id.get_name().unwrap_or("unnamed")
            )
        })?;

        let (min, max) = elem.get_playback_volume_range();

        let volume_steps = (max - min) as f64;
        let normalised_volume = if self.linear_scaling {
            (((volume as f64) / (u16::MAX as f64)) * volume_steps) as i64 + min
        } else {
            ((volume as f64 + 1.0).log((u16::MAX as f64) + 1.0) * volume_steps).floor() as i64 + min
        };

        elem.set_playback_volume_all(normalised_volume)?;
        Ok(())
    }
}

impl Mixer for AlsaMixer {
    fn open(_: MixerConfig) -> AlsaMixer {
        AlsaMixer {
            device: "default".to_string(),
            mixer: "Master".to_string(),
            linear_scaling: false,
        }
    }

    fn volume(&self) -> u16 {
        let selem_id = alsa::mixer::SelemId::new(&self.mixer, 0);
        let mixer = alsa::mixer::Mixer::new(&self.device, false).ok();
        let vol = mixer
            .as_ref()
            .and_then(|mixer| mixer.find_selem(&selem_id))
            .and_then(|elem| {
                let (min, max) = elem.get_playback_volume_range();
                elem.get_playback_volume(alsa::mixer::SelemChannelId::mono())
                    .ok()
                    .map(|volume| {
                        (((volume - min) as f64 / (max - min) as f64) * (u16::MAX as f64)).floor()
                            as u16
                    })
            });
        match vol {
            Some(vol) => vol,
            None => {
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
}
