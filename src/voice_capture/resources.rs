//! Admitted local application artifacts. No wire request supplies these paths.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
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
    pub(super) fn executable(&self) -> PathBuf {
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
pub struct Assets {
    root: PathBuf,
    files: Vec<VerifiedFile>,
}
impl Assets {
    /// Snapshot only the small helper graph. No runtime/model copy, recursive
    /// deletion or adoption of an existing destination is performed.
    pub fn capture(directory: &Path, descriptor: &DescriptorArtifact) -> io::Result<Self> {
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
        let rows = [
            (
                "electron-capture-main.cjs",
                include_bytes!("../../ui/voice/electron-capture-main.cjs").as_slice(),
            ),
            (
                "electron-capture-preload.cjs",
                include_bytes!("../../ui/voice/electron-capture-preload.cjs").as_slice(),
            ),
            (
                "electron-capture-renderer.mjs",
                include_bytes!("../../ui/voice/electron-capture-renderer.mjs").as_slice(),
            ),
            (
                "browser-capture-owner.mjs",
                include_bytes!("../../ui/voice/browser-capture-owner.mjs").as_slice(),
            ),
            (
                "NativePcmCapture.js",
                include_bytes!("../../ui/js/NativePcmCapture.js").as_slice(),
            ),
        ];
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
        })
    }
    pub(super) fn entry(&self) -> PathBuf {
        self.root.join("electron-capture-main.cjs")
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
