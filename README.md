# Spotifyd <!-- omit in toc -->

[![Cargo Downloads](https://img.shields.io/crates/d/spotifyd)](https://crates.io/crates/spotifyd)
[![Dependabot Status][dependabot-badge]](https://dependabot.com)
[![Github Actions - CD][cd-badge]][github-actions]
[![Github Actions - CI][ci-badge]][github-actions]

> An open source Spotify client running as a UNIX daemon.

Spotifyd streams music just like the official client, but is more lightweight and supports more platforms. Spotifyd also supports the Spotify Connect protocol, which makes it show up as a device that can be controlled from the official clients.

> __Note:__ Spotifyd requires a Spotify Premium account.

__To read about how to install and configure Spotifyd, take a look at our [wiki][wiki]!__

- [Common issues](#common-issues)
- [Contributing](#contributing)
- [Credits](#credits)

## Common issues

- Spotifyd will not work without Spotify Premium
- The device name cannot contain spaces

## Contributing

We always appreciate help during the development of `spotifyd`! If you are new to programming, open source or Rust in general, take a look at issues tagged with [`good first issue`][good-first-issues]. These normally are easy to resolve and don't take much time to implement.

## Credits

This project would not have been possible without the amazing reverse engineering work done in [librespot](https://github.com/librespot-org/librespot), mostly by [plietar](https://github.com/plietar).

<!-- This section contains all links used within the document. This prevents cluttering and makes reading the raw markdown a lot easier -->
[github-actions]: https://github.com/Spotifyd/spotifyd/actions
[good-first-issues]: https://github.com/Spotifyd/spotifyd/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22
[cd-badge]: https://github.com/Spotifyd/spotifyd/workflows/Continuous%20Deployment/badge.svg
[ci-badge]: https://github.com/Spotifyd/spotifyd/workflows/Continuous%20Integration/badge.svg
[dependabot-badge]: https://api.dependabot.com/badges/status?host=github&repo=Spotifyd/spotifyd
[wiki]: https://spotifyd.github.io/spotifyd/
