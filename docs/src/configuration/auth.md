# Authentication

There are two different ways of authentication supported.

## Discovery on LAN

By default, `spotifyd` advertises itself on the local network as a Spotify Connect device and will thus appear in **official** clients. After selecting it over there, you should be able to hear the sound coming out of your `spotifyd` device and see respective logs.

For this to work, you need to make sure that your firewall isn't blocking the discovery. In particular, `spotifyd` uses two ports:

- `5353 UDP`: MDNS service advertisement
- A zeroconf port which uses TCP. By default, it is randomly chosen, but if you want to, you can configure it with the `--zeroconf-port` cli option / `zeroconf_port` config value.

> __Note:__ If you choose to go with this, it is also recommended to omit the `cache_path` and `cache_directory` options. Otherwise the first user to connect to the service will have its authentication blob cached by the service and nobody else will be able to connect to the service without clearing the cache.

## Username+Password Authentication

If you don't want everyone on your network to be able to connect, want to control `spotifyd` from outside your network or don't want to use the official clients, you can also make `spotifyd` directly connect to your account. For this to work, you need to pass username and password to `spotifyd`. There are different ways to configure them.

### Plaintext in config

The easiest way is to put the values for `username` and `password` in the config file.

### By command

You can replace `username` and/or `password` with a `username_cmd` or `password_cmd`. `spotifyd` will then execute the provided command at startup and use the result as the value. This way, you can for example use `password_cmd = "pass show spotify.com"` if you're managing your passwords with `pass`.

### Keyring

One of the safest ways to store your password is within the system keyring.

> __Note:__ If choosing the user's keyring to store login credentials, running spotifyd as a systemd _system service_ is no longer possible. A system wide service cannot access a specific user's keyring. In this case, make sure to run spotifyd as a systemd _user service_. See [systemd configuration](../advanced/systemd.md).

This features leverages [Linux's DBus Secret Service API](https://www.freedesktop.org/wiki/Specifications/secret-storage-spec/) or native macOS keychain in order to forgo the need to store your password directly in the config file. To use it, compile with the `dbus_keyring` feature and set the `use-keyring` config entry to `true` or pass the `--use-keyring` CLI flag  during start to the daemon. Also, remove the `password` and/or `password_cmd` config entries.

Your keyring entry needs to have the following attributes set:

```yaml
application: rust-keyring
service: spotifyd
username: <your-spotify-username>
```

To add such an entry into your keyring, you can use `secret-tool`, a CLI used to communicate with agents that support the Secret Service API:

```bash
secret-tool store --label='name you choose' application rust-keyring service spotifyd username <your-username>
```

You can use the keychain GUI on macOS to add an item respectively, or with the built-in `security` tool:

```bash
security add-generic-password -s spotifyd -D rust-keyring -a <your username> -w
```

To allow `spotifyd` to find the entry, you must set `username` to the same value that you entered above.

### From the cache

If you have a `cache_path` set in your config, an authentication blob (not the real credentials) will be stored for the last successful authentication. You can look at it at `<cache_path>/credentials.json`. If the configured username matches the one in the `credentials.json`, `spotifyd` will use this auth blob instead of re-authenticating. So instead of leaving the password in the config file, you can instead remove it after the first successful authentication.

> __Note:__ An authentication blob will also be saved if you're using discovery, so you could authenticate once using discovery and going from there just use the cache method.

To always use the cached credentials (not only, when the usernames match), you can configure the `username_cmd` this way:

```toml
username_cmd = "jq -r .username <cache_path>/credentials.json"
# or, a version without `jq`
username_cmd = "grep -oP '(?<=\"username\":\")[^\"]*' <cache_path>/credentials.json"
```
