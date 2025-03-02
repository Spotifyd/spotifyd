# Running as a service

Most people want to have `spotifyd` always running in the background. The preferred method to do this depends on the OS you're using.

## Linux

If you installed `spotifyd` directly from your distribution, chances are that the installation includes a service definition such that you can run

```console
$ systemctl --user start spotifyd # start spotifyd using systemd
$ systemctl --user enable --now spotifyd # start spotifyd and enable starting on login
```

If you installed `spotifyd` some other way, head over to [the advanced section](../advanced/systemd.md) for further instructions.

## macOS

If you installed `spotifyd` using brew, the following commands should do the trick

```console
$ brew services run spotifyd # start the service once
$ brew services start spotifyd # start spotifyd and enable starting on boot
```

If you installed `spotifyd` without brew, [the advanced section](../advanced/launchd.md) has got you covered.

## FreeBSD

When installed via the package manager, the following commands are available:

```console
$ sudo service spotifyd onestart # start spotifyd once
$ sudo sysrc spotifyd_enable=YES # enable starting spotifyd on boot
```
