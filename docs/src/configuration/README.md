# Configuration

`Spotifyd` is able to run without configuration at all and will assume default values for most of the fields. However, running without configuration will only allow you to connect to it via Spotify Connect if you're on the same network as the daemon.

First, you need to create a config file. `spotifyd` will look for one at `/etc/spotifyd.conf`, `$XDG_CONFIG_PATH/spotifyd/spotifyd.conf` or if that is not set `~/.config/spotifyd/spotifyd.conf`. For other needs, you can point `spotifyd` to it's config file with `--config-path <path-to-your-config>`.

You can start with the following documented config as an example and read through the subpages of this section:
```toml
{{#include ../../../contrib/spotifyd.conf}}
```
