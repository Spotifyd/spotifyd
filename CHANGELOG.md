# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added 
### Changed
### Removed

## [0.3.3]
### Added 
- Add `debug_credentials` feature for debugging `BadCredentials` errors [#915]
- Implement `VolumeUp` and `VolumeDown` in the DBUS/MPRIS interface [#963]
- Update librespot to 0.2.0 [#977]
- Rewrite DBUS/MPRIS integration [#977]
### Changed
- Improved panic error message [#925]
### Removed

[#915]: https://github.com/Spotifyd/spotifyd/pull/915
[#925]: https://github.com/Spotifyd/spotifyd/pull/925
[#963]: https://github.com/Spotifyd/spotifyd/pull/963
[#977]: https://github.com/Spotifyd/spotifyd/pull/977

## [0.3.1]
### Added 
- Use eyre for better error reporting [#789]
- Add a contributers file

### Changed
- Change docs from readme to mkdocs [#783]
- Update librespot, thus fixing [#719] [#900]
### Removed

[#789]: https://github.com/Spotifyd/spotifyd/pull/789
[#783]: https://github.com/Spotifyd/spotifyd/pull/783
[#719]: https://github.com/Spotifyd/spotifyd/issues/719
[#900]: https://github.com/Spotifyd/spotifyd/pull/900
## [0.3.0]
### Added
- Added a changelog [#714]
### Changed
- Changed the config format from ini to TOML [#571]

[#571]: https://github.com/Spotifyd/spotifyd/pull/571
[#714]: https://github.com/Spotifyd/spotifyd/pull/714
### Removed

## [0.2.24]

[Unreleased]: https://github.com/Spotifyd/spotifyd/compare/v0.2.24...HEAD
[0.2.24]: https://github.com/Spotifyd/spotifyd/releases/tag/v0.2.24