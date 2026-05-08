---
name: Bug report
about: Create a report to help us improve
title: ''
labels: bug
assignees: ''

---

**Description**
<!-- Please write a clear and concise description about the bug -->

**To Reproduce**
<!-- Steps to reproduce the behavior: -->
1. 
2. 
3. 
4. 

**Expected behavior**
<!-- Please write a clear and concise description of what you have expected to happen (if applicable) -->

**Logs**
<details><summary>Click to show logs</summary>

```
<!-- PLEASE PASTE YOUR LOGS BELOW THIS LINE WHEN REPORTING BUGS. Make sure to run spotifyd using the `--verbose` flag -->
<!-- They have to be inside of the backticks '```'. --> 
```

</details>

<!-- If you compiled Spotifyd yourself. Alsa backend enabled by default unless compiled with the `--no-default-features` flag. Standard users don't need to change this. -->
**Compilation flags** 
- [ ] dbus_mpris
- [x] alsa_backend
- [ ] portaudio_backend
- [ ] pulseaudio_backend
- [ ] rodio_backend

**Versions (please complete the following information):**
<!-- DO NOT use words like `latest`. Please specify the exact version/commit hash -->
- OS: <!-- e.g. Debian 13, Fedora 44, Windows 10 -->
- Spotifyd: <!-- release version or commit hash (spotifyd --version) -->
- cargo: <!-- cargo --version -->

<!-- **Additional context**
Add any other context about the problem that might be helpful for fixing the bug here. -->
