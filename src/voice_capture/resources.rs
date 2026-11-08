//! Admitted local application artifacts. No wire request supplies these paths.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::fd::AsRawFd,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

fn invalid() -> io::Error {
    io::Error::other("Native capture artifact admission failed")
}
#[derive(Clone, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
    len: u64,
    modified: i64,
    modified_ns: i64,
    changed: i64,
    changed_ns: i64,
    mode: u32,
}
fn identity(metadata: &fs::Metadata) -> io::Result<Identity> {
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o222 != 0
    {
        return Err(invalid());
    }
    Ok(Identity {
        dev: metadata.dev(),
        ino: metadata.ino(),
        len: metadata.len(),
        modified: metadata.mtime(),
        modified_ns: metadata.mtime_nsec(),
        changed: metadata.ctime(),
        changed_ns: metadata.ctime_nsec(),
        mode: metadata.mode(),
    })
}
fn canonical(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || path.canonicalize()? != path {
        return Err(invalid());
    }
    Ok(())
}
fn relative(path: &str) -> io::Result<&Path> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(invalid());
    }
    Ok(path)
}
struct VerifiedFile {
    path: PathBuf,
    file: File,
    identity: Identity,
}
impl VerifiedFile {
    fn admit(path: PathBuf, expected: &str, size: u64) -> io::Result<Self> {
        canonical(&path)?;
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)?;
        let before = identity(&file.metadata()?)?;
        if before.len != size {
            return Err(invalid());
        }
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 65536];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if format!("{:x}", hash.finalize()) != expected || identity(&file.metadata()?)? != before {
            return Err(invalid());
        }
        let admitted = Self {
            path,
            file,
            identity: before,
        };
        admitted.verify()?;
        Ok(admitted)
    }
    fn verify(&self) -> io::Result<()> {
        canonical(&self.path)?;
        if identity(&fs::symlink_metadata(&self.path)?)? != self.identity
            || identity(&self.file.metadata()?)? != self.identity
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Deserialize)]
struct Member {
    path: String,
    bytes: u64,
    sha256: String,
    executable: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    version: String,
    platform: String,
    archive_sha256: String,
    files: Vec<Member>,
}

/// A retained lease over the exact original application graph. File hashes are
/// compiled from a source-controlled manifest independently derived from the
/// checksum-pinned archive, never trusted from a mutable acquisition sidecar.
pub struct Runtime {
    root: PathBuf,
    files: Vec<VerifiedFile>,
}
impl Runtime {
    pub fn admit(root: &Path) -> io::Result<Self> {
        canonical(root)?;
        let manifest: Manifest = serde_json::from_str(include_str!(
            "../../ui/voice/electron-runtime-44.10.1-linux-x64.json"
        ))
        .map_err(|_| invalid())?;
        if manifest.version != "44.10.1"
            || manifest.platform != "linux-x64"
            || manifest.archive_sha256
                != "2be868b71b0aa71f30e6b5567a106a35a049385e963183c074aad1975ead5167"
            || manifest.files.len() != 72
        {
            return Err(invalid());
        }
        let mut files = Vec::new();
        let mut expected = BTreeSet::new();
        for member in manifest.files {
            if !expected.insert(PathBuf::from(relative(&member.path)?)) {
                return Err(invalid());
            }
            let file = VerifiedFile::admit(
                root.join(relative(&member.path)?),
                &member.sha256,
                member.bytes,
            )?;
            if (file.identity.mode & 0o111 != 0) != member.executable {
                return Err(invalid());
            }
            files.push(file);
        }
        graph(root, &expected)?;
        Ok(Self {
            root: root.into(),
            files,
        })
    }
    pub(crate) fn executable(&self) -> PathBuf {
        self.root.join("electron")
    }
    pub fn verify(&self) -> io::Result<()> {
        for file in &self.files {
            file.verify()?
        }
        graph(
            &self.root,
            &self
                .files
                .iter()
                .map(|file| {
                    file.path
                        .strip_prefix(&self.root)
                        .map(Path::to_path_buf)
                        .map_err(|_| invalid())
                })
                .collect::<io::Result<BTreeSet<_>>>()?,
        )
    }
}
fn graph(root: &Path, expected: &BTreeSet<PathBuf>) -> io::Result<()> {
    let mut directories = BTreeSet::new();
    for file in expected {
        let mut parent = file.parent();
        while let Some(path) = parent {
            if path.as_os_str().is_empty() {
                break;
            }
            directories.insert(path.to_path_buf());
            parent = path.parent();
        }
    }
    let mut seen = BTreeSet::new();
    let mut pending = vec![PathBuf::new()];
    let mut count = 0;
    while let Some(relative) = pending.pop() {
        let directory = root.join(&relative);
        let metadata = fs::symlink_metadata(&directory)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o022 != 0
        {
            return Err(invalid());
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let name = relative.join(entry.file_name());
            count += 1;
            if count > expected.len() + directories.len() {
                return Err(invalid());
            }
            let kind = entry.file_type()?;
            if kind.is_dir() && directories.contains(&name) {
                pending.push(name)
            } else if kind.is_file() && expected.contains(&name) {
                seen.insert(name);
            } else {
                return Err(invalid());
            }
        }
    }
    if &seen != expected {
        return Err(invalid());
    }
    Ok(())
}

/// A native installer/build receipt for this application's existing N-API
/// descriptor helper. This is an internal factory input, not request data.
pub struct DescriptorArtifact {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}
impl DescriptorArtifact {
    /// Read-only admission for package checks; never load or execute the addon.
    pub fn verify(&self) -> io::Result<()> {
        VerifiedFile::admit(self.path.clone(), &self.sha256, self.bytes)?.verify()
    }
}

/// One application-owned Chromium identity, never an existing browser profile.
/// The lease survives every capture using it and excludes a second application
/// process. Chromium's local identity persists; each capture gates media anew.
pub struct Profile {
    path: PathBuf,
    directory_identity: (u64, u64),
    lock: File,
    lock_identity: (u64, u64),
}
impl Profile {
    pub fn open(path: &Path) -> io::Result<Self> {
        let parent = path.parent().ok_or_else(invalid)?;
        canonical(parent)?;
        let metadata = fs::symlink_metadata(parent)?;
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            return Err(invalid());
        }
        match fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error),
        }
        let directory_identity = Self::check_directory(path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path.join("owner.lock"))?;
        let identity = Self::lock_identity(&lock.metadata()?)?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::other("voice_profile_busy"));
        }
        let profile = Self {
            path: path.into(),
            directory_identity,
            lock,
            lock_identity: identity,
        };
        profile.verify()?;
        Ok(profile)
    }
    fn check_directory(path: &Path) -> io::Result<(u64, u64)> {
        canonical(path)?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            return Err(invalid());
        }
        Ok((metadata.dev(), metadata.ino()))
    }
    fn lock_identity(metadata: &fs::Metadata) -> io::Result<(u64, u64)> {
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            return Err(invalid());
        }
        Ok((metadata.dev(), metadata.ino()))
    }
    pub fn verify(&self) -> io::Result<()> {
        if Self::check_directory(&self.path)? != self.directory_identity
            || Self::lock_identity(&self.lock.metadata()?)? != self.lock_identity
            || Self::lock_identity(&fs::symlink_metadata(self.path.join("owner.lock"))?)?
                != self.lock_identity
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}
pub struct Assets {
    root: PathBuf,
    files: Vec<VerifiedFile>,
    entry: &'static str,
}
impl Assets {
    /// Snapshot only the small helper graph. No runtime/model copy, recursive
    /// deletion or adoption of an existing destination is performed.
    pub fn capture(directory: &Path, descriptor: &DescriptorArtifact) -> io::Result<Self> {
        Self::install(
            directory,
            descriptor,
            "electron-capture-main.cjs",
            &[
                (
                    "electron-capture-main.cjs",
                    include_bytes!("../../ui/voice/electron-capture-main.cjs"),
                ),
                (
                    "electron-capture-preload.cjs",
                    include_bytes!("../../ui/voice/electron-capture-preload.cjs"),
                ),
                (
                    "electron-capture-renderer.mjs",
                    include_bytes!("../../ui/voice/electron-capture-renderer.mjs"),
                ),
                (
                    "browser-capture-owner.mjs",
                    include_bytes!("../../ui/voice/browser-capture-owner.mjs"),
                ),
                (
                    "browser-microphones.mjs",
                    include_bytes!("../../ui/voice/browser-microphones.mjs"),
                ),
                (
                    "NativePcmCapture.js",
                    include_bytes!("../../ui/js/NativePcmCapture.js"),
                ),
            ],
        )
    }
    /// Install the original consent producer and its private metadata adapter.
    /// This graph contains no capture engine or audio/control network client.
    pub fn consent(directory: &Path, descriptor: &DescriptorArtifact) -> io::Result<Self> {
        Self::install(
            directory,
            descriptor,
            "consent-helper.cjs",
            &[
                (
                    "consent-helper.cjs",
                    include_bytes!("../../ui/voice/consent-helper.cjs"),
                ),
                (
                    "consent-storage.cjs",
                    include_bytes!("../../ui/voice/consent-storage.cjs"),
                ),
                (
                    "native-consent.cjs",
                    include_bytes!("../../ui/voice/native-consent.cjs"),
                ),
            ],
        )
    }
    fn install(
        directory: &Path,
        descriptor: &DescriptorArtifact,
        entry: &'static str,
        rows: &[(&str, &[u8])],
    ) -> io::Result<Self> {
        let parent = directory.parent().ok_or_else(invalid)?;
        canonical(parent)?;
        let metadata = fs::symlink_metadata(parent)?;
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            return Err(invalid());
        }
        fs::DirBuilder::new().mode(0o700).create(directory)?;
        if descriptor.bytes > 1024 * 1024 {
            return Err(invalid());
        }
        let helper = VerifiedFile::admit(
            descriptor.path.clone(),
            &descriptor.sha256,
            descriptor.bytes,
        )?;
        let mut files = Vec::new();
        for (name, bytes) in rows {
            files.push(snapshot(directory, name, bytes)?)
        }
        let bytes = fs::read(&descriptor.path)?;
        helper.verify()?;
        if format!("{:x}", Sha256::digest(&bytes)) != descriptor.sha256 {
            return Err(invalid());
        }
        files.push(snapshot(directory, "electron-private-fd.node", &bytes)?);
        Ok(Self {
            root: directory.into(),
            files,
            entry,
        })
    }
    pub(crate) fn entry(&self) -> PathBuf {
        self.root.join(self.entry)
    }
    pub(crate) fn descriptor(&self) -> PathBuf {
        self.root.join("electron-private-fd.node")
    }
    pub fn verify(&self) -> io::Result<()> {
        for file in &self.files {
            file.verify()?
        }
        Ok(())
    }
}
fn snapshot(directory: &Path, name: &str, bytes: &[u8]) -> io::Result<VerifiedFile> {
    let path = directory.join(name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    file.set_permissions(fs::Permissions::from_mode(0o400))?;
    drop(file);
    VerifiedFile::admit(
        path,
        &format!("{:x}", Sha256::digest(bytes)),
        bytes.len() as u64,
    )
}

#[cfg(test)]
mod profile_tests {
    use super::*;
    use std::{os::unix::fs::symlink, sync::Arc};

    fn fixture() -> PathBuf {
        assert!(Path::new("/run/.containerenv").exists());
        let root = std::env::temp_dir().join(format!("voice-profile-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        root
    }
    fn clear(root: &Path) {
        // Delete only the known files created by this fixture, never a tree.
        fs::remove_file(root.join("profile/owner.lock")).unwrap();
        fs::remove_dir(root.join("profile")).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn retained_profile_lease_excludes_a_second_owner() {
        let root = fixture();
        let profile = Arc::new(Profile::open(&root.join("profile")).unwrap());
        let capture = profile.clone();
        drop(profile);
        assert!(Profile::open(&root.join("profile")).is_err());
        capture.verify().unwrap();
        drop(capture);
        drop(Profile::open(&root.join("profile")).unwrap());
        clear(&root);
    }

    #[test]
    fn profile_never_adopts_a_symlink_to_another_fixture_directory() {
        let root = fixture();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(root.join("other"))
            .unwrap();
        symlink(root.join("other"), root.join("profile")).unwrap();
        assert!(Profile::open(&root.join("profile")).is_err());
        fs::remove_file(root.join("profile")).unwrap();
        fs::remove_dir(root.join("other")).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn profile_rejects_public_modes_and_replaced_lock_identity() {
        let root = fixture();
        let path = root.join("profile");
        let profile = Profile::open(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(profile.verify().is_err());
        assert!(Profile::open(&path).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        fs::rename(path.join("owner.lock"), path.join("retired.lock")).unwrap();
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path.join("owner.lock"))
            .unwrap();
        assert!(profile.verify().is_err());
        drop(profile);
        fs::remove_file(path.join("retired.lock")).unwrap();
        clear(&root);
    }

    #[test]
    fn profile_rejects_replaced_directory_even_when_the_lock_is_preserved() {
        let root = fixture();
        let path = root.join("profile");
        let retired = root.join("retired");
        let profile = Profile::open(&path).unwrap();
        let lock_identity = profile.lock_identity;
        fs::rename(&path, &retired).unwrap();
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::rename(retired.join("owner.lock"), path.join("owner.lock")).unwrap();
        assert_eq!(
            Profile::lock_identity(&fs::symlink_metadata(path.join("owner.lock")).unwrap())
                .unwrap(),
            lock_identity
        );
        assert!(profile.verify().is_err());
        drop(profile);
        fs::remove_dir(retired).unwrap();
        clear(&root);
    }
}
