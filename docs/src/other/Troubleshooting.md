# Troubleshooting on FreeBSD

On FreeBSD 11.2, I installed `spotifyd` via the official binary package from the FreeBSD repos.
I configured it accordingly with `backend = portaudio`, but I wasn't getting any sound.  The issue was that the device I needed to output sound to is not PortAudio's default.  I don't know how PortAudio determines its default, and I initially couldn't figure out the format or device names that spotifyd wanted in its configuration.  Here are some pointers that may help you if you find yourself in the same situation.

If you have `portaudio` as your backend and set your device to `?`, spotifyd will output all of the available PortAudio devices it can find on your system.  For mine, that looks like this:

```bash
- /dev/dsp0 (default)
- /dev/dsp1
- /dev/dsp2
- /dev/dsp
```

The 4th device (index 3, starting from 0) (`/dev/dsp`) is the output device that I needed.  That coincides with the `pcm3` (also index 3, starting from 0) device that FreeBSD lists in `dmesg` as well as  the `hw.snd.default_unit=3` sysctl that I use to set the device as OSS's default.  It seems like the index number correlates across each of those enumerations.

Once I set `device = "/dev/dsp"` (quotes here seem necessary) in the config, the sound started working.  It may take some experimentation to find the correct output on other systems.