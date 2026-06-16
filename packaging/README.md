# Packaging

## AppImage (Linux desktop app)

Builds a single portable `Phanes-<version>-x86_64.AppImage` of the desktop app
(`phanes-ui`) that runs on most modern Linux desktops without installation.

### Prerequisites

- A Rust toolchain (`cargo`).
- [`linuxdeploy`](https://github.com/linuxdeploy/linuxdeploy) and
  [`appimagetool`](https://github.com/AppImage/appimagetool) on `PATH`.
- FUSE (to *run* the AppImage). On systems without FUSE, run it with
  `./Phanes-*.AppImage --appimage-extract-and-run`.

### Build

```bash
packaging/build-appimage.sh
```

The result lands in `dist/Phanes-<version>-x86_64.AppImage`. Run it directly:

```bash
./dist/Phanes-*.AppImage            # opens ./ideas by default
./dist/Phanes-*.AppImage ~/notes    # or point it at a folder
```

### What's inside / notes

- Built with `--features ui,enrich`, so the AI features (Scan + AI, Ask, bridges,
  questions) work when a local OpenAI-compatible model server is running. Without a
  server the deterministic features still work — the model layer is opt-in (INV-1).
  The model client uses pure-Rust `rustls` (not OpenSSL), so nothing TLS-related is
  bundled and model calls work inside the AppImage (BUG-004).
- The server defaults to `http://127.0.0.1:1234` (LM Studio's default). To point
  elsewhere, set `PHANES_LLM_URL` / `PHANES_LLM_MODEL` / `PHANES_EMBED_URL` /
  `PHANES_EMBED_MODEL` in the environment that launches the AppImage — note that a
  desktop-menu launch does **not** inherit your shell profile, so for non-default
  setups launch from a terminal (or set the vars in the `.desktop` entry).
- `phanes-ui` takes the ideas folder as its first argument and defaults to
  `ideas` relative to the working directory. Launched from a desktop menu the
  working directory is usually `$HOME`, so it will look for `~/ideas`; launch
  from a terminal (or pass a path) to point it elsewhere.
- `linuxdeploy` bundles the binary's shared-library dependencies and correctly
  leaves OpenGL/driver libraries to the host. egui uses `winit`, which `dlopen`s
  a few libraries at runtime (e.g. `libxkbcommon`, `libwayland-client`); these
  are present on typical desktops. On a minimal system, install those packages or
  bundle them explicitly if the AppImage fails to start.
- The CLI (`phanes`) is not packaged here — it's a normal `cargo build --release`
  binary with no GUI dependencies.

### Assets

- `appimage/phanes.desktop` — the desktop entry.
- `appimage/phanes.png` — the 256×256 app icon.
- `build-appimage.sh` — the build script.
