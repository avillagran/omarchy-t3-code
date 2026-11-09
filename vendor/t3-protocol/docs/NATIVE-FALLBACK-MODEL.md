# Native fallback-model setting

The native settings descriptor and patch registry expose the nullable
`fallbackModelSelection` field from T3 Code nightly4301, commit
`4cb6945dc60ed03f567dc5daafd083e52308e82b`. The label and description follow
`ProjectDefaultsSettings.tsx`. Its scope is the server; the field is shown only
when the server reports it. It is used for new threads when the default model's
provider is out of usage.

The existing model-option and current-registry validation applies to writes;
reset sends null. The read descriptor strips unrelated credential fields.
This is an additive settings key: Rust signatures, C ABI and Android/JNI/Kotlin
sources remain unchanged. The actual fallback decision belongs to the native
new-thread selection helper and its caller; adding this setting alone does not
claim that consumer behavior.

Private `android-sandbox` acceptance covers the full Rust suite, the native C ABI
and actual JNI calls. A dedicated native-settings case checks nullable reset,
registry rejection and credential-free presentation. The JNI worktree uses a
separate Cargo/CMake output directory to retain the older build cache without
reusing its source-directory identity. No host JVM tests are run.
