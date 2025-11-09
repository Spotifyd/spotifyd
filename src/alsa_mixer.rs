use color_eyre::eyre::{self, Context, eyre};
use librespot_playback::mixer::{Mixer, MixerConfig};
use log::error;
use std::sync::{Arc, Mutex, MutexGuard};

pub struct AlsaMixer {
    pub mixer: Arc<Mutex<alsa::Mixer>>,
    pub config: MixerConfig,
}

impl AlsaMixer {
    fn get_selem<'a>(
        &'a self,
        lock: &'a MutexGuard<'a, alsa::Mixer>,
    ) -> eyre::Result<alsa::mixer::Selem<'a>> {
        let selem_id = alsa::mixer::SelemId::new(&self.config.control, self.config.index);
        let selem = lock.find_selem(&selem_id).ok_or_else(|| {
            eyre!(
                "No control with name '{}' in alsa device '{}'",
                self.config.control,
                self.config.device,
            )
        })?;
        Ok(selem)
    }
    fn set_volume_with_err(&self, volume: u16) -> eyre::Result<()> {
        let lock = self.mixer.lock().expect("lock shouldn't be poisoned");
        let elem = self.get_selem(&lock)?;
        let (min, max) = elem.get_playback_volume_range();

        let volume_steps = (max - min) as f64;
        let normalised_volume = if matches!(
            self.config.volume_ctrl,
            librespot_playback::config::VolumeCtrl::Linear
        ) {
            (((volume as f64) / (u16::MAX as f64)) * volume_steps) as i64 + min
        } else {
            ((volume as f64 + 1.0).log((u16::MAX as f64) + 1.0) * volume_steps).floor() as i64 + min
        };

        elem.set_playback_volume_all(normalised_volume)?;
        Ok(())
    }

    fn get_volume_with_err(&self) -> eyre::Result<u16> {
        let lock = self.mixer.lock().expect("lock shouldn't be poisoned");
        let elem = self.get_selem(&lock)?;
        let (min, max) = elem.get_playback_volume_range();
        let volume_steps = (max - min) as f64;
        let vol = elem.get_playback_volume(alsa::mixer::SelemChannelId::mono())?;
        let normalized_volume = if matches!(
            self.config.volume_ctrl,
            librespot_playback::config::VolumeCtrl::Linear
        ) {
            ((vol - min) as f64 * u16::MAX as f64 / volume_steps).floor() as u16
        } else {
            ((u16::MAX as f64 + 1.0).powf(((vol - min) as f64) / volume_steps) - 1.0).floor() as u16
        };
        Ok(normalized_volume)
    }
}

impl Mixer for AlsaMixer {
    fn open(config: MixerConfig) -> Result<AlsaMixer, librespot_core::Error> {
        let mixer = alsa::Mixer::new(&config.device, false)
            .wrap_err("failed to open mixer")
            .map_err(librespot_core::Error::invalid_argument)?;
        Ok(AlsaMixer {
            mixer: Arc::new(Mutex::new(mixer)),
            config,
        })
    }

    fn volume(&self) -> u16 {
        match self.get_volume_with_err() {
            Ok(vol) => vol,
            Err(err) => {
                error!("failed to get volume from alsa device: {err:?}");
                0
            }
        }
    }

    fn set_volume(&self, volume: u16) {
        match self.set_volume_with_err(volume) {
            Ok(_) => (),
            Err(err) => error!("Couldn't set volume of alsa device: {err:?}"),
        }
    }
}
