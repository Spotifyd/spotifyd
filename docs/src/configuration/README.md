# Configuration

`Spotifyd` is able to run without configuration at all and will assume default values for most of the fields. However, running without configuration will only allow you to connect to it via Spotify Connect if you're on the same network as the daemon.

All configuration options can be specified directly as command line arguments or alternatively (in particular for permanent changes) be written into a configuration file. Throughout this section, we will always give both possibilities, e.g. `--cli-arg` for the cli variant / `config_value` for the config file variable.

## Config File

`spotifyd` will look for its configuration at `/etc/spotifyd.conf`, `$XDG_CONFIG_PATH/spotifyd/spotifyd.conf` or if that is not set `~/.config/spotifyd/spotifyd.conf`. For other needs, you can point `spotifyd` to it's config file with the command line argument `--config-path <path-to-your-config>`.

You can start with the following documented config as an example and read through the subpages of this section:
```toml
{{#include ../../../contrib/spotifyd.conf}}
```
