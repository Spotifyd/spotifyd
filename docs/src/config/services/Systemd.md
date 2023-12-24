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

To run the service on boot:

```bash
sudo loginctl enable-linger <username>
```

Where <username> is the user name which starts the service, the user wich runned thw systemctl command.
The command is required to enable your user to run long-running services. Without it `systemd` would kill the `spotifyd` process as soon as you log out, and only run it when you log in.

## As a system wide service

A `systemd.service` unit file is provided to help run spotifyd as a service on systemd-based systems. The file `contrib/spotifyd.service` should be copied to:

```bash
/etc/systemd/system/
```

Control of the daemon is handed over to systemd. The following example commands will start the service and keep it running across reboots.

```bash
systemctl daemon-reload
systemctl enable spotifyd.service --now
```
