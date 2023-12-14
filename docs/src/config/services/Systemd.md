# Running spotifyd as a systemd service

## As as a user service

A `systemd.service` unit file is provided to help run spotifyd as a service on systemd-based systems. The file `contrib/spotifyd.service` should be copied to either:

```bash
/etc/systemd/user/
~/.config/systemd/user/
```

Packagers of systemd-based distributions are encouraged to include the file in the former location. End-user should prefer the latter. It should be noted that some targets are not available when running under the user directory, such as `network-online.target`.

Control of the daemon is handed over to systemd. The following command will start the service whenever the user logs in to the system. Logging out will stop the service.

```bash
systemctl --user enable spotifyd.service --now
```

## As a system wide service

<div class="warning">

When running spotifyd as a system wide service, it is not possible to access a user's keyring to obtain login credentials. Do not set `use_keyring = true` and do not specify `--use-keyring`, when running as a system wide service. To be able to access login credentials stored in the user's keyring, run spotifyd as a user service, as decribed above.

Additionally, `use_mpris = true` or `--use-mpris` should not be used, since their intended usage is within user sessions (and not system-wide daemons). If you have very specific requirements and still want to control a system-wide `spotifyd` instance, there is some help available [here](https://github.com/Spotifyd/spotifyd/issues/244).

</div>

A `systemd.service` unit file is provided to help run spotifyd as a service on systemd-based systems. The file `contrib/spotifyd.service` should be copied to:

```bash
/etc/systemd/system/
```

Control of the daemon is handed over to systemd. The following example commands will start the service and keep it running across reboots.

```bash
systemctl daemon-reload
systemctl enable spotifyd.service --now
```
