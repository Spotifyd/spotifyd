# Running as launchd service

On macOS, the system wide and per-user daemon/agent manager is known as `launchd`. Interfacing with `launchd` is performed through `launchctl`.

In order to use `spotifyd` as a service on macOS one must specify a `.plist` that represents the service, and place it in `/Library/LaunchDaemons`.

Here is a .plist which works with macOS Catalina 10.15.3:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
    <dict>
		<key>Label</key>
		<string>rustlang.spotifyd</string>
		<key>ProgramArguments</key>
		<array>
			<string>/usr/local/bin/spotifyd</string>
			<string>--config-path=/users/YourUserName/.config/spotifyd/spotifyd.conf</string>
			<string>--no-daemon</string>
		</array>
		<key>UserName</key>
		<string>YourUserName</string>
		<key>KeepAlive</key>
		<true/>
		<key>ThrottleInterval</key>
		<integer>30</integer>
	</dict>
</plist>
```

Once present in the `/Library/LaunchDaemons` directory, the .plist must be loaded and started with the following commands.

`sudo launchctl load -w /Library/LaunchDaemons/rustlang.spotifyd.plist`


`sudo launchctl start /Library/LaunchDaemons/rustlang.spotifyd.plist`

One may also unload/stop the service in a similar fashion replacing load/start with unload/stop.

Note:

* You should update "YourUserName" with your actual username for macOS (or remove "UserName" to run as root.

* The string, `<string>--no-daemon</string>` is needed as launchd won't receive a PID for the process and will lose its remit over spotifyd. So it's best to include it, there will be no difference in use, nor will you see any log output.

* macOS tries to start the daemon immediately on boot, and spotifyd fails if Wifi isn't connected. So one must have a keep alive (which retries if it fails to launch on boot), that retries after 30 seconds, which is enough for wifi etc to come up.
