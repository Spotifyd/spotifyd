---
name: Bug report
about: Create a report to help us improve
title: ''
labels: bug
assignees: ''

---

**Description**
<!-- A clear and concise description of what the bug is -->

**To Reproduce**
<!-- Steps to reproduce the behavior:
1. Go to '...'
2. Click on '....'
3. Scroll down to '....'
4. See error -->

**Expected behavior**
<!-- A clear and concise description of what you expected to happen (if applicable) -->

**Logs**
<details><summary>Click to show logs</summary>
<!-- PLEASE PASTE YOUR LOGS BELOW THIS LINE WHEN REPORTING BUGS. Make sure to run spotifyd using the `--verbose` flag -->
<!-- They have to be betwenn the `/summary` and the `/details` HTML tags -->  

</details>

<!-- if you compiled spotifyd yourself. Alsa backend enabled by default unless compiled with the `--no-default-features` flag -->
**Compilation flags** 
- [ ] dbus_mpris
- [ ] dbus_keyring
- [x] alsa_backend
- [ ] portaudio_backend
- [ ] pulseaudio_backend
- [ ] rodio_backend

**Versions (please complete the following information):**
 - OS: <!-- e.g. Ubuntu 18.04 LTS, Windows 10 -->
 - Spotifyd: <!-- commit hash or release version -->
- cargo: <!-- cargo --version -->

<!-- **Additional context**
Add any other context about the problem here. -->
