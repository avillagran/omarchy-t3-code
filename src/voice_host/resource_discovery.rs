//! Select native startup resources without admitting or executing their files.
//! Bundle metadata is compiled into the client, never read from an adjacent receipt.
use std::{ffi::OsString, io, path::PathBuf};

const OVERRIDE_KEYS: [&str; 4] = [
    "O3_NATIVE_VOICE_RUNTIME",
    "O3_NATIVE_VOICE_ADDON",
    "O3_NATIVE_VOICE_ADDON_SHA256",
    "O3_NATIVE_VOICE_ADDON_BYTES",
];
const RUNTIME_DIRECTORY: &str = "electron-44.10.1-linux-x64";
const MAX_ADDON_BYTES: u64 = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Configuration {
    pub runtime: PathBuf,
    pub addon: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

fn invalid() -> io::Error {
    io::Error::other("voice_resources_configuration_invalid")
}

fn pin(sha256: &str, bytes: &str) -> io::Result<(String, u64)> {
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || bytes.is_empty()
        || !bytes.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid());
    }
    let bytes = bytes.parse::<u64>().map_err(|_| invalid())?;
    if bytes == 0 || bytes > MAX_ADDON_BYTES {
        return Err(invalid());
    }
    Ok((sha256.to_owned(), bytes))
}

fn absolute(path: PathBuf) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(invalid());
    }
    // Runtime and addon admission subsequently reject symlinks and noncanonical
    // paths, verify ownership/modes, and retain the complete file identities.
    Ok(path)
}

fn resolve(
    overrides: [Option<OsString>; 4],
    bundled: [Option<&str>; 2],
    supported: bool,
    executable: io::Result<PathBuf>,
) -> io::Result<Option<Configuration>> {
    let supplied = overrides.iter().filter(|value| value.is_some()).count();
    if supplied != 0 && supplied != OVERRIDE_KEYS.len() {
        // An incomplete explicit configuration must never inherit bundled files.
        return Err(invalid());
    }
    let selected = if supplied == OVERRIDE_KEYS.len() {
        let [runtime, addon, sha256, bytes] = overrides.map(Option::unwrap);
        let (sha256, bytes) = pin(
            sha256.to_str().ok_or_else(invalid)?,
            bytes.to_str().ok_or_else(invalid)?,
        )?;
        Configuration {
            runtime: absolute(runtime.into())?,
            addon: absolute(addon.into())?,
            sha256,
            bytes,
        }
    } else {
        let (sha256, bytes) = match bundled {
            [None, None] => return Ok(None),
            [Some(sha256), Some(bytes)] => pin(sha256, bytes)?,
            _ => return Err(invalid()),
        };
        if !supported {
            return Err(io::Error::other("voice_resources_platform_unsupported"));
        }
        let executable = absolute(executable?)?;
        let directory = executable
            .parent()
            .ok_or_else(invalid)?
            .join("native-voice");
        Configuration {
            runtime: directory.join(RUNTIME_DIRECTORY),
            addon: directory.join("electron-private-fd.node"),
            sha256,
            bytes,
        }
    };
    if !supported {
        return Err(io::Error::other("voice_resources_platform_unsupported"));
    }
    Ok(Some(selected))
}

pub(super) fn configured() -> io::Result<Option<Configuration>> {
    resolve(
        OVERRIDE_KEYS.map(std::env::var_os),
        [
            option_env!("O3_NATIVE_VOICE_BUNDLED_ADDON_SHA256"),
            option_env!("O3_NATIVE_VOICE_BUNDLED_ADDON_BYTES"),
        ],
        cfg!(all(target_os = "linux", target_arch = "x86_64")),
        std::env::current_exe(),
    )
}

pub(super) fn bundled() -> io::Result<Option<Configuration>> {
    resolve(
        std::array::from_fn(|_| None),
        [
            option_env!("O3_NATIVE_VOICE_BUNDLED_ADDON_SHA256"),
            option_env!("O3_NATIVE_VOICE_BUNDLED_ADDON_BYTES"),
        ],
        cfg!(all(target_os = "linux", target_arch = "x86_64")),
        std::env::current_exe(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const SHA256: &str = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";

    fn absent() -> [Option<OsString>; 4] {
        std::array::from_fn(|_| None)
    }

    fn explicit() -> [Option<OsString>; 4] {
        [
            "/overrides/electron",
            "/overrides/addon.node",
            SHA256,
            "4096",
        ]
        .map(|s| Some(s.into()))
    }

    fn executable() -> io::Result<PathBuf> {
        Ok("/application/bin/client".into())
    }

    #[test]
    fn absent_configuration_does_not_require_an_executable_or_supported_target() {
        assert_eq!(
            resolve(absent(), [None, None], false, Err(invalid())).unwrap(),
            None
        );
    }

    #[test]
    fn bundle_paths_are_relative_to_the_client_and_not_the_working_directory() {
        let actual = resolve(absent(), [Some(SHA256), Some("4096")], true, executable())
            .unwrap()
            .unwrap();
        assert_eq!(
            actual.runtime,
            Path::new("/application/bin/native-voice/electron-44.10.1-linux-x64")
        );
        assert_eq!(
            actual.addon,
            Path::new("/application/bin/native-voice/electron-private-fd.node")
        );
        assert_eq!(actual.sha256, SHA256);
        assert_eq!(actual.bytes, 4096);
    }

    #[test]
    fn complete_override_wins_even_when_bundle_or_executable_is_invalid() {
        let actual = resolve(explicit(), [Some("invalid"), None], true, Err(invalid()))
            .unwrap()
            .unwrap();
        assert_eq!(actual.runtime, Path::new("/overrides/electron"));
        assert_eq!(actual.addon, Path::new("/overrides/addon.node"));
    }

    #[test]
    fn every_partial_override_is_rejected_without_a_bundle_fallback() {
        for mask in 1..15 {
            let mut overrides = explicit();
            for (index, value) in overrides.iter_mut().enumerate() {
                if mask & (1 << index) == 0 {
                    *value = None;
                }
            }
            assert!(resolve(overrides, [Some(SHA256), Some("4096")], true, executable()).is_err());
        }
    }

    #[test]
    fn incomplete_or_invalid_bundled_pins_are_rejected() {
        for metadata in [
            [Some(SHA256), None],
            [None, Some("4096")],
            [Some(""), Some("4096")],
            [Some(SHA256), Some("0")],
            [Some(SHA256), Some("1048577")],
            [Some(SHA256), Some("18446744073709551616")],
            [Some(SHA256), Some("+4096")],
        ] {
            assert!(resolve(absent(), metadata, true, executable()).is_err());
        }
    }

    #[test]
    fn invalid_override_does_not_fall_back_to_a_valid_bundle() {
        for (index, value) in [(0, "relative/runtime"), (1, ""), (2, "bad"), (3, "-1")] {
            let mut overrides = explicit();
            overrides[index] = Some(value.into());
            assert!(resolve(overrides, [Some(SHA256), Some("4096")], true, executable()).is_err());
        }
    }

    #[test]
    fn non_utf8_override_metadata_is_rejected() {
        use std::os::unix::ffi::OsStringExt;
        let mut overrides = explicit();
        overrides[2] = Some(OsString::from_vec(vec![0xff]));
        assert!(resolve(overrides, [None, None], true, executable()).is_err());
    }

    #[test]
    fn only_the_pinned_runtime_target_can_be_selected() {
        assert!(resolve(explicit(), [None, None], false, executable()).is_err());
        assert!(resolve(absent(), [Some(SHA256), Some("4096")], false, executable()).is_err());
    }

    #[test]
    fn bundled_discovery_requires_an_absolute_executable() {
        assert!(resolve(absent(), [Some(SHA256), Some("4096")], true, Err(invalid())).is_err());
        assert!(resolve(
            absent(),
            [Some(SHA256), Some("4096")],
            true,
            Ok("relative/client".into())
        )
        .is_err());
    }
}
