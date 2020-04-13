build:
	cargo build --release --no-default-features --features "pulseaudio_backend,dbus_keyring,dbus_mpris"

run:
	cargo run --no-default-features --features "pulseaudio_backend,dbus_keyring,dbus_mpris" -- --no-daemon --verbose

install: ./target/release/spotifyd
	sudo cp ./target/release/spotifyd /usr/bin/
