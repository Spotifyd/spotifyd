# Running spotifyd as a service using systemd

A `systemd.service` unit file is provided to help run spotifyd as a service on systemd-based systems. The file `contrib/spotifyd.service` should be copied to either:

```bash
/etc/systemd/user/
~/.config/systemd/user/
```

Packagers of systemd-based distributions are encouraged to include the file in the former location. End-user should prefer the latter. It should be noted that some targets are not available when running under the user directory, such as `network-online.target`.

Control of the daemon is handed over to systemd. The following example commands will run the service once and enable the service to always run on login in the future respectively:

```bash
systemctl --user start spotifyd.service
systemctl --user enable spotifyd.service
```
