//! Linux cookie readers ported from pinned T3 BrowserImport modules. These
//! private-controller primitives have no CLI/IPC route returning cookie data.
use crate::{
    browser_cookie_database::{Cell, Database},
    Result,
};
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::Path};
use zeroize::{Zeroize, Zeroizing};
pub struct ImportedCookie {
    pub url: String,
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub expiration_date: Option<f64>,
    pub same_site: &'static str,
}
impl Drop for ImportedCookie {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}
pub struct CookieRead {
    pub cookies: Vec<ImportedCookie>,
    pub undecryptable: usize,
    pub undecryptable_hosts: Vec<String>,
}
impl CookieRead {
    fn new() -> Self {
        Self {
            cookies: vec![],
            undecryptable: 0,
            undecryptable_hosts: vec![],
        }
    }
}
fn cookie(
    host: &str,
    name: &str,
    value: &str,
    path: &str,
    secure: bool,
    http_only: bool,
    expiration_date: Option<f64>,
    same_site: &'static str,
) -> ImportedCookie {
    let bare = host.strip_prefix('.').unwrap_or(host);
    let authority = if bare.contains(':') && !(bare.starts_with('[') && bare.ends_with(']')) {
        format!("[{bare}]")
    } else {
        bare.to_owned()
    };
    ImportedCookie {
        url: format!(
            "{}://{authority}{path}",
            if secure { "https" } else { "http" }
        ),
        name: name.into(),
        value: value.into(),
        domain: host.starts_with('.').then(|| host.into()),
        path: path.into(),
        secure,
        http_only,
        expiration_date,
        same_site,
    }
}
fn same_site(value: Option<f64>) -> &'static str {
    match value {
        Some(0.) => "no_restriction",
        Some(1.) => "lax",
        Some(2.) => "strict",
        _ => "unspecified",
    }
}
pub fn read_firefox(path: &Path) -> Result<CookieRead> {
    let db = Database::snapshot(path)?;
    let versions = db.rows("pragma user_version")?;
    let version = versions
        .first()
        .and_then(|r| r.first())
        .ok_or("readFailed")?
        .number()?;
    let raw = if (10. ..=14.).contains(&version) {
        "rawSameSite"
    } else {
        "null"
    };
    let rows=db.rows(&format!("select host,name,value,path,expiry,isSecure,isHttpOnly,sameSite,{raw} from moz_cookies where originAttributes = ''"))?;
    let mut out = CookieRead::new();
    for r in rows {
        let expiry = r[4].number()?;
        let expiration = if expiry <= 0. {
            None
        } else {
            Some(if version >= 16. {
                (expiry / 1000.).floor()
            } else {
                expiry
            })
        };
        let value = r[7].optional_number()?;
        let raw = r[8].optional_number()?;
        out.cookies.push(cookie(
            r[0].text()?,
            r[1].text()?,
            r[2].text()?,
            r[3].text()?,
            r[5].number()? == 1.,
            r[6].number()? == 1.,
            expiration,
            if value == Some(1.) && raw == Some(0.) {
                "unspecified"
            } else {
                same_site(value)
            },
        ));
    }
    Ok(out)
}
pub struct LinuxKeys {
    v10: Zeroizing<[u8; 16]>,
    v11: Option<Zeroizing<[u8; 16]>>,
    empty: Zeroizing<[u8; 16]>,
    v11_error: Option<String>,
}
fn derive(secret: &[u8]) -> Zeroizing<[u8; 16]> {
    let mut key = Zeroizing::new([0u8; 16]);
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(secret, b"saltysalt", 1, &mut *key);
    key
}
impl LinuxKeys {
    /// Invoke only from a confirmed import; the helper retains the normal OS unlock prompt.
    /// Stdout stays private to Rust, never to the watch output or a page process.
    pub fn from_helper(helper: &Path, application: &str) -> Result<Self> {
        use std::{
            io::Read,
            os::unix::process::CommandExt,
            process::{Command, Stdio},
        };
        if !matches!(
            application,
            "chrome" | "msedge" | "brave" | "vivaldi" | "opera" | "chromium"
        ) {
            return Err("unknownSource".into());
        }
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut command = Command::new(helper);
        command
            .arg(application)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env_clear()
            .env("PATH", "/usr/bin:/bin");
        for key in [
            "HOME",
            "DBUS_SESSION_BUS_ADDRESS",
            "XDG_RUNTIME_DIR",
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "LANG",
            "LC_ALL",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        let parent = unsafe { libc::getpid() };
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != parent
                {
                    return Err(std::io::Error::other("browser_parent_gone"));
                }
                Ok(())
            });
        }
        let secret = (|| -> Result<Zeroizing<String>> {
            let mut child = Child(command.spawn().map_err(|_| "keychainUnavailable")?);
            let mut bytes = Zeroizing::new(Vec::new());
            child
                .0
                .stdout
                .take()
                .ok_or("keychainUnavailable")?
                .take(65537)
                .read_to_end(&mut bytes)
                .map_err(|_| "keychainUnavailable")?;
            if bytes.len() > 65536 {
                return Err("keychainUnavailable".into());
            }
            let status = child.0.wait().map_err(|_| "keychainUnavailable")?;
            match status.code() {
                Some(0) if !bytes.is_empty() => {
                    Ok(Zeroizing::new(String::from_utf8_lossy(&bytes).into_owned()))
                }
                Some(0 | 2) => Err("keychainItemMissing".into()),
                Some(3) => Err("needsKeychainApproval".into()),
                _ => Err("keychainUnavailable".into()),
            }
        })();
        Self::from_secret(
            secret
                .as_ref()
                .map(|s| Some(s.as_str()))
                .map_err(|e| e.clone()),
        )
    }
    /// Only the explicitly invoked import controller may obtain a real OS secret.
    /// Consent refusal fails before even v10 cookies are read, matching original.
    pub fn from_secret(secret: Result<Option<&str>>) -> Result<Self> {
        let (v11, error) = match secret {
            Ok(Some(secret)) => (Some(derive(secret.as_bytes())), None),
            Ok(None) => (None, None),
            Err(e) if e == "needsKeychainApproval" => return Err(e),
            Err(e) => (None, Some(e)),
        };
        Ok(Self {
            v10: derive(b"peanuts"),
            v11,
            empty: derive(b""),
            v11_error: error,
        })
    }
}
fn bound(bytes: &[u8], host: &str, version: u32) -> Option<String> {
    let bytes = if version >= 24 {
        let hash = Sha256::digest(host.as_bytes());
        if bytes.len() < 32 || bytes[..32] != hash[..] {
            return None;
        }
        &bytes[32..]
    } else {
        bytes
    };
    Some(String::from_utf8_lossy(bytes).into_owned())
}
fn cbc(payload: &[u8], key: &[u8; 16], host: &str, version: u32) -> Option<String> {
    let mut bytes = Zeroizing::new(payload.to_vec());
    let plain = cbc::Decryptor::<aes::Aes128>::new(key.into(), b"                ".into())
        .decrypt_padded_mut::<Pkcs7>(&mut bytes)
        .ok()?;
    bound(plain, host, version)
}
pub fn decrypt_linux(
    encrypted: &[u8],
    keys: &LinuxKeys,
    host: &str,
    version: u32,
) -> Option<String> {
    if encrypted.is_empty() {
        return Some(String::new());
    }
    let prefix = encrypted.get(..3);
    let key = match prefix {
        Some(b"v10") => Some(&keys.v10),
        Some(b"v11") => keys.v11.as_ref(),
        _ => return bound(encrypted, host, version),
    }?;
    cbc(&encrypted[3..], key, host, version)
        .or_else(|| cbc(&encrypted[3..], &keys.empty, host, version))
}
pub fn read_chromium(path: &Path, keys: &LinuxKeys) -> Result<CookieRead> {
    let db = Database::snapshot(path)?;
    let meta = db.rows("select value from meta where key = 'version' limit 1")?;
    let version = meta.first().and_then(|r| r.first()).ok_or("readFailed")?;
    let number = match version {
        Cell::Text(s) => s.parse::<f64>().map_err(|_| "readFailed")?,
        _ => version.number()?,
    };
    if !number.is_finite() || number < 0. || number.fract() != 0. || number > u32::MAX as f64 {
        return Err("readFailed".into());
    }
    let version = number as u32;
    let partition = if version >= 15 {
        "top_frame_site_key"
    } else {
        "''"
    };
    let rows=db.rows(&format!("select host_key,name,value,encrypted_value,path,expires_utc / 1000000,is_secure,is_httponly,samesite,{partition} from cookies"))?;
    let mut out = CookieRead::new();
    let mut hosts = HashSet::new();
    let mut needed_v11 = false;
    for r in rows {
        let host = r[0].text()?;
        let encrypted = r[3].blob()?;
        let partitioned = !r[9].text()?.is_empty();
        // Validate every row before producing a partial success, as the original schema decoder does.
        let name = r[1].text()?;
        let plaintext = r[2].text()?;
        let path = r[4].text()?;
        let expires = r[5].number()?;
        let secure = r[6].number()? == 1.;
        let http_only = r[7].number()? == 1.;
        let site = same_site(Some(r[8].number()?));
        needed_v11 |= !partitioned && encrypted.starts_with(b"v11");
        let value = if partitioned {
            None
        } else if encrypted.is_empty() {
            Some(plaintext.to_owned())
        } else {
            decrypt_linux(encrypted, keys, host, version)
        };
        match value {
            Some(value) => {
                let value = Zeroizing::new(value);
                out.cookies.push(cookie(
                    host,
                    name,
                    &value,
                    path,
                    secure,
                    http_only,
                    if expires <= 0. {
                        None
                    } else {
                        Some(expires - 11_644_473_600.)
                    },
                    site,
                ));
            }
            None => {
                out.undecryptable += 1;
                let host = host.strip_prefix('.').unwrap_or(host);
                if hosts.insert(host.to_owned()) {
                    out.undecryptable_hosts.push(host.into())
                }
            }
        }
    }
    if out.cookies.is_empty() && needed_v11 {
        if let Some(error) = &keys.v11_error {
            return Err(error.clone());
        }
    }
    Ok(out)
}

/// Private import-controller entry point; callers still own target admission,
/// reservation and acknowledgement. This function never publishes cookie data.
pub fn read_selected(
    home: &Path,
    source_id: &str,
    directory: &str,
    confirmed: bool,
    helper: &Path,
) -> Result<CookieRead> {
    if !confirmed {
        return Err("confirmation_required".into());
    }
    let selected = crate::browser_sources::resolve_import_at(home, source_id, directory)?;
    if let Some(application) = selected.application {
        let keys = LinuxKeys::from_helper(helper, application)?;
        // An unlock dialog may have been open while the browser was reopened.
        let fresh = crate::browser_sources::resolve_import_at(home, source_id, directory)?;
        if fresh.database != selected.database {
            return Err("readFailed".into());
        }
        read_chromium(&fresh.database, &keys)
    } else {
        read_firefox(&selected.database)
    }
}
