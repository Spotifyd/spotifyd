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

Now, you're ready to run `spotifyd authenticate` (for available options, see `spotifyd auth --help`).
This will ask you to browse to a link with your preferred web browser. On the page, you need to log into your Spotify account and confirm the connection.

If the process was successful, you should see the message "Go back to your terminal :)" in your browser window. You can now close the tab and return to the terminal.

Now, when running `spotifyd --no-daemon`, you should see that `spotifyd` automatically connects to your account.

```
Loading config from "..."
[...]
Login via OAuth as user <your username>.
[...]
Authenticated as '<your username>' !
```

> __Note:__ Even if you logged into `spotifyd` using this method, discovery will still be enabled by default and any incoming connection will interrupt the current session. If you don't want or need this, you can disable it via the `--disable-discovery` cli option / `disable_discovery = true` config value.
