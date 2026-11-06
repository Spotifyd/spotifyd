# Authentication

There are two different ways of authentication supported.

## Discovery on LAN

By default, `spotifyd` advertises itself on the local network as a Spotify Connect device and will thus appear in **official** clients. After selecting it over there, you should be able to hear the sound coming out of your `spotifyd` device.

For this to work, you need to make sure that your firewall isn't blocking the discovery. In particular, `spotifyd` uses two ports:

- `5353 UDP`: MDNS service advertisement
- A zeroconf port which uses TCP. By default, it is randomly chosen, but if you want to, you can configure it with the `--zeroconf-port` cli option / `zeroconf_port` config value.

If you don't want discovery, because you're using one of the methods below, you can disable it via the `--disable-discovery` cli option / `disable_discovery = true` config value.

> __Note:__ By default, the last active session will be remembered and reconnected once the service is restarted.

## Manual Login (via OAuth)

If for some reason, discovery is not a viable option for your use case or you prefer a single-user instance, you can manually log in to your account and `spotifyd` will connect to this account by default.

> __Note:__ Since the login method requires a web browser, trying this on headless systems is not recommended. As a workaround, you can log in on a machine with a working web browser and then copy the credential file onto the headless system. The location of the credential file will be `<cache_path>/oauth/credentials.json`.

Before you begin the login flow, make sure that you won't need to change `cache_path` later on, because that location will be used to store the login data.

For a single-user setup, you can configure a dedicated credential location in `spotifyd.conf`:

```toml
[global]
cache_path = "/home/your-user/.cache/spotifyd"
```

Then run `spotifyd authenticate` and complete the OAuth flow in your browser. The credentials are stored in `<cache_path>/oauth/credentials.json`; do not create or edit that file manually. Spotifyd reads its configuration from TOML, not JSON.

The accompanying [JSON profile example](./oauth-profile.json) is a non-secret reference for the same settings only. Spotifyd does not load it.

This asks you to open a link in your preferred web browser, log into Spotify, and confirm the connection. For available options, run `spotifyd authenticate --help`.

If the process was successful, you should see the message "Go back to your terminal :)" in your browser window. You can now close the tab and return to the terminal.

When running `spotifyd --no-daemon`, spotifyd should connect to your account:

```
Loading config from "..."
[...]
Login via OAuth as user <your username>.
[...]
Authenticated as '<your username>' !
```

### Using Spotify clients, including the web player

OAuth authorizes spotifyd; it does not sign the browser into Spotify Web Player or make spotifyd play audio inside the browser. To use the web player, open [open.spotify.com](https://open.spotify.com/) and sign in there separately. With spotifyd running and Connect discovery enabled, select `spotifyd` from the web player's device picker to play through spotifyd.

The web player and spotifyd need to be able to discover each other on the local network. If `spotifyd` does not appear, check that discovery is enabled and that your firewall allows mDNS (`5353/UDP`) and spotifyd's zeroconf TCP port. Setting `disable_discovery = true` (or passing `--disable-discovery`) prevents Spotify clients, including the web player, from connecting to spotifyd.

### Web Playback SDK quick-start player

The Spotify Web Playback SDK is a separate browser integration; it does not connect to spotifyd. A minimal player setup looks like this:

```js
window.onSpotifyWebPlaybackSDKReady = () => {
  const player = new Spotify.Player({
    name: "Web Playback SDK Quick Start Player",
    getOAuthToken: (cb) => {
      getSpotifyAccessToken()
        .then((token) => cb(token))
        .catch((error) => console.error("Could not get Spotify access token:", error));
    },
    volume: 0.5,
  });

  player.connect();
};
```

Load the Spotify Web Playback SDK before this code and implement `getSpotifyAccessToken()` in your application to obtain and refresh a valid Spotify access token securely. Do not hard-code a token or use spotifyd's cached credentials here. The SDK has its own account, permission, and application requirements; see Spotify's [Web Playback SDK guide](https://developer.spotify.com/documentation/web-playback-sdk).
