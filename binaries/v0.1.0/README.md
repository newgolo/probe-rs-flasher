# Binaries

Prebuilt, single-file executables. These are convenience copies of the artifacts produced by
the build scripts (`build-linux.sh` / `build-windows.sh`); GitHub Releases also carry them when
the release workflow succeeds.

| File | Platform | Notes |
| --- | --- | --- |
| `probe-rs-flasher` | Linux x86_64 | requires glibc; X11/Wayland/OpenGL are loaded at runtime |
| `probe-rs-flasher.exe` | Windows 10/11 x64 | no MinGW runtime required |
| `*.sha256` | — | SHA-256 checksums |

Verify a download with:

```bash
sha256sum -c probe-rs-flasher.sha256
```
