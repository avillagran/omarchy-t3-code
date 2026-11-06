# Rust tests in a private container

Build the actual browser renderer, then compile and run the Rust suite:

```sh
tools/build-browser-viewer
tools/test-rust-container
```

Select integration targets with repeated `--test` options:

```sh
tools/test-rust-container --test browser_guest --test browser_import
```

The runner requires rootless Podman, the cached Arch Linux image, Node, Rust and
the normal Qt development dependencies. `CARGO_TARGET_DIR` is respected. Cargo
compiles on the host; test executables run only inside the container.

Source, Git metadata, compiled binaries, Qt libraries and fonts are read-only.
Each test binary receives its own temporary home and XDG directories. No live
display, session bus, browser profile or device is mounted, and external networking
is disabled. Chromium retains its own sandbox; its namespace syscalls are allowed
inside the outer container. The browser guest is tested with actual Qt rendering,
DOM input and private cookie storage.

Evidence is retained under a unique `/var/tmp/o3-client-rust-*` directory. It
includes compilation diagnostics, selected executables, test output and the final
result. Runtime profiles stay in container tmpfs. The runner never removes host
file trees. Do not commit raw evidence containing synthetic endpoint credentials.
