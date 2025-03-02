# Troubleshooting

## no sound on FreeBSD

If you have correctly configured `spotifyd` to use `portaudio` and everything seems to be working except that there's no sound, you might have to switch to a different audio device.

If you have `portaudio` as your backend and set your device to `?`, spotifyd will output all of the available PortAudio devices it can find on your system.  This could look like the following

```bash
- /dev/dsp0 (default)
- /dev/dsp1
- /dev/dsp2
- /dev/dsp
```

Assume that the 4th device (index 3, starting from 0, `/dev/dsp`) is the output device that is needed. That coincides with the `pcm3` (also index 3, starting from 0) device that FreeBSD lists in `dmesg` as well as the `hw.snd.default_unit=3` sysctl that is used to set the device as OSS's default.  It seems like the index number correlates across each of those enumerations.

After setting `device = "/dev/dsp"` in the config, the sound should start working. If not, you can try the other possible values that are available.
