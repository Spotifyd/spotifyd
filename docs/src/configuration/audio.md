# Audio Configuration

On most setups, audio should be Just Working™. If you have specific needs or something's not working, you might need to touch some of the config values.

## Backend

> `-b/--backend` or `backend` in the config file.

There are different audio backends available to choose from. See `spotifyd --help` to get the available names.

## Device selection

> `--device` or `device` in the config file.

Instead of using the default device, which can sometimes be different to what you'd prefer, you can ask `spotifyd` to use a specific audio device. The interpretation and available values depends on the backend you're using.

- For ALSA, you can use `aplay -L` to get a list of possible devices.
- For PulseAudio, run `pactl list short sinks` to get a list of possible names.

## Bitrate

> `-B/--bitrate` or `bitrate` in the config file.

To reduce bandwidth usage or increase quality, you can play with the bitrate.

## Volume Controller

> `--volume-controller` or `volume_controller` in the config file.

In most cases, leaving this at the default (`softvol`) should be fine.

If you want your `spotifyd` volume to be synchronized with an output device's hardware volume, you can set this to `alsa` or `alsa_linear`. In both cases, you might also want to set the `mixer` device to set which device's volume should be changed.

If you want to prevent the user to be able to adjust the volume, set this instead to `none`.

## Other

For more interesting but less relevant audio options, have a look at `spotifyd --help` or [the example config](./).
