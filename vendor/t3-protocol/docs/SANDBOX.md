# Isolated compilation and tests

All Android compilation and tests, including JNI/JVM fixtures, run through
`tools/android-sandbox`. CI also runs Rust and C ABI checks through that command.
The wrapper requires rootless Podman and rejects broad project/state roots.
The build, Kotlin, JNI and package entrypoints reject execution outside the
container before compiling or creating outputs.

Build the image with rootless Podman:

```sh
podman --remote=false build -t localhost/t3-protocol-android-sandbox:local -f .github/actions/android-sandbox/Containerfile .github/actions/android-sandbox
```

Set `ANDROID_SANDBOX_HOME` to a dedicated private state directory and
`ANDROID_SANDBOX_SDK` to an installed SDK. Set `ANDROID_SANDBOX_IMAGE` to that
image. Its embedded JDK is `/opt/java/openjdk`. Explicit toolchain jars can be
mounted read-only with colon-separated `ANDROID_SANDBOX_RO` paths. Supply the
Kotlin compiler and Android toolchain variables documented in ANDROID.md.

The project worktree is mounted read-write, including for generated source
outputs. The SDK and optional toolchain inputs are read-only. Private Cargo,
Gradle, target and home state is writable; `/tmp` and `/run` use private tmpfs.
The wrapper does not prevent source writes. Each build or test must limit those
writes to its declared outputs and retain source changes for review.
The container drops capabilities, disables privilege escalation and has no
network by default. It receives no host authentication or display credentials.
Artifacts and package staging directories are retained for inspection.

Acquire locked Cargo dependencies separately, then run checks offline:

```sh
ANDROID_SANDBOX_NETWORK=pasta CARGO_NET_OFFLINE=false tools/android-sandbox --sh 'cargo fetch --locked'
tools/android-sandbox --sh 'cargo test --locked'
tools/android-sandbox --sh 'tools/test-c-abi'
tools/android-sandbox --sh 'tools/test-jni'
tools/android-sandbox --sh 'tools/test-kotlin-jni'
```

Network acquisition is not permission to run project tests outside the sandbox.
The composite CI action builds this same image and exposes `android-sandbox` on
PATH. Release and ordinary test workflows wrap every project build/runtime step.
No Android emulator or device lifecycle coverage is implied by JVM fixtures.
