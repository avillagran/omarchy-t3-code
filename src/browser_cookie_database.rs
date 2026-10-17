//! Private, transactionally consistent SQLite cookie snapshots. No source writes.
use crate::Result;
use std::{
    ffi::CString,
    os::unix::ffi::OsStrExt,
    path::Path,
    time::{Duration, Instant},
};
use zeroize::Zeroize;
type Raw = *mut libc::c_void;
#[link(name = "sqlite3")]
unsafe extern "C" {
    fn sqlite3_open_v2(
        path: *const libc::c_char,
        db: *mut Raw,
        flags: libc::c_int,
        vfs: *const libc::c_char,
    ) -> libc::c_int;
    fn sqlite3_close(db: Raw) -> libc::c_int;
    fn sqlite3_prepare_v2(
        db: Raw,
        sql: *const libc::c_char,
        n: libc::c_int,
        stmt: *mut Raw,
        tail: *mut *const libc::c_char,
    ) -> libc::c_int;
    fn sqlite3_step(stmt: Raw) -> libc::c_int;
    fn sqlite3_column_count(stmt: Raw) -> libc::c_int;
    fn sqlite3_column_type(stmt: Raw, column: libc::c_int) -> libc::c_int;
    fn sqlite3_column_double(stmt: Raw, column: libc::c_int) -> f64;
    fn sqlite3_column_text(stmt: Raw, column: libc::c_int) -> *const u8;
    fn sqlite3_column_blob(stmt: Raw, column: libc::c_int) -> *const libc::c_void;
    fn sqlite3_column_bytes(stmt: Raw, column: libc::c_int) -> libc::c_int;
    fn sqlite3_finalize(stmt: Raw) -> libc::c_int;
    fn sqlite3_backup_init(
        to: Raw,
        to_name: *const libc::c_char,
        from: Raw,
        from_name: *const libc::c_char,
    ) -> Raw;
    fn sqlite3_backup_step(backup: Raw, pages: libc::c_int) -> libc::c_int;
    fn sqlite3_backup_finish(backup: Raw) -> libc::c_int;
    fn sqlite3_progress_handler(
        db: Raw,
        n: libc::c_int,
        callback: Option<unsafe extern "C" fn(Raw) -> libc::c_int>,
        state: Raw,
    );
}
pub enum Cell {
    Text(String),
    Blob(Vec<u8>),
    Number(f64),
    Null,
}
impl Drop for Cell {
    fn drop(&mut self) {
        match self {
            Self::Text(s) => s.zeroize(),
            Self::Blob(b) => b.zeroize(),
            _ => {}
        }
    }
}
impl Cell {
    pub fn text(&self) -> Result<&str> {
        match self {
            Self::Text(s) => Ok(s),
            _ => Err("readFailed".into()),
        }
    }
    pub fn number(&self) -> Result<f64> {
        match self {
            Self::Number(n) if n.is_finite() => Ok(*n),
            _ => Err("readFailed".into()),
        }
    }
    pub fn optional_number(&self) -> Result<Option<f64>> {
        if matches!(self, Self::Null) {
            Ok(None)
        } else {
            self.number().map(Some)
        }
    }
    pub fn blob(&self) -> Result<&[u8]> {
        match self {
            Self::Blob(b) => Ok(b),
            _ => Err("readFailed".into()),
        }
    }
}
pub struct Database(Raw);
impl Drop for Database {
    fn drop(&mut self) {
        unsafe {
            sqlite3_close(self.0);
        }
    }
}
struct Statement(Raw);
impl Drop for Statement {
    fn drop(&mut self) {
        unsafe {
            sqlite3_finalize(self.0);
        }
    }
}
struct Progress(Raw);
impl Drop for Progress {
    fn drop(&mut self) {
        unsafe {
            sqlite3_progress_handler(self.0, 0, None, std::ptr::null_mut());
        }
    }
}
impl Database {
    fn open(path: &std::ffi::CStr, flags: i32) -> Result<Self> {
        unsafe {
            let mut raw = std::ptr::null_mut();
            let status = sqlite3_open_v2(path.as_ptr(), &mut raw, flags, std::ptr::null());
            if raw.is_null() {
                return Err("readFailed".into());
            }
            let db = Self(raw);
            if status != 0 {
                return Err("readFailed".into());
            }
            Ok(db)
        }
    }
    pub fn snapshot(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Err("readFailed".into());
        }
        let name = CString::new(path.as_os_str().as_bytes()).map_err(|_| "readFailed")?;
        let source = Self::open(&name, 1)?;
        let target = Self::open(c":memory:", 2 | 4)?;
        // SQLite backup includes committed WAL pages and provides a coherent point in time.
        // The source connection is read-only; the destination never touches a user file.
        unsafe {
            let backup =
                sqlite3_backup_init(target.0, c"main".as_ptr(), source.0, c"main".as_ptr());
            if backup.is_null() {
                return Err("readFailed".into());
            }
            let start = Instant::now();
            let status = loop {
                let s = sqlite3_backup_step(backup, 256);
                if s != 0 || start.elapsed() > Duration::from_secs(5) {
                    break s;
                }
            };
            let finish = sqlite3_backup_finish(backup);
            if status != 101 || finish != 0 {
                return Err("readFailed".into());
            }
        }
        Ok(target)
    }
    pub fn rows(&self, sql: &str) -> Result<Vec<Vec<Cell>>> {
        let sql = CString::new(sql).map_err(|_| "readFailed")?;
        unsafe extern "C" fn deadline(state: Raw) -> libc::c_int {
            i32::from((&*(state as *const Instant)).elapsed() > Duration::from_secs(5))
        }
        unsafe {
            let start = Instant::now();
            sqlite3_progress_handler(
                self.0,
                1000,
                Some(deadline),
                (&start as *const Instant).cast_mut().cast(),
            );
            let _progress = Progress(self.0);
            let mut stmt = std::ptr::null_mut();
            let status =
                sqlite3_prepare_v2(self.0, sql.as_ptr(), -1, &mut stmt, std::ptr::null_mut());
            if stmt.is_null() {
                return Err("readFailed".into());
            }
            let stmt = Statement(stmt);
            if status != 0 {
                return Err("readFailed".into());
            }
            let columns = sqlite3_column_count(stmt.0);
            let mut rows = vec![];
            let mut total = 0usize;
            loop {
                match sqlite3_step(stmt.0) {
                    101 => return Ok(rows),
                    100 => {}
                    _ => return Err("readFailed".into()),
                }
                if rows.len() >= 100_000 {
                    return Err("readFailed".into());
                }
                let mut row = vec![];
                for c in 0..columns {
                    row.push(match sqlite3_column_type(stmt.0, c) {
                        1 | 2 => Cell::Number(sqlite3_column_double(stmt.0, c)),
                        5 => Cell::Null,
                        kind @ (3 | 4) => {
                            let length = sqlite3_column_bytes(stmt.0, c);
                            if !(0..=1024 * 1024).contains(&length) {
                                return Err("readFailed".into());
                            }
                            total += length as usize;
                            if total > 64 * 1024 * 1024 {
                                return Err("readFailed".into());
                            }
                            let p = if kind == 3 {
                                sqlite3_column_text(stmt.0, c)
                            } else {
                                sqlite3_column_blob(stmt.0, c).cast()
                            };
                            if p.is_null() && length > 0 {
                                return Err("readFailed".into());
                            }
                            let bytes = if length == 0 {
                                &[][..]
                            } else {
                                std::slice::from_raw_parts(p, length as usize)
                            };
                            if kind == 3 {
                                Cell::Text(String::from_utf8_lossy(bytes).into_owned())
                            } else {
                                Cell::Blob(bytes.to_vec())
                            }
                        }
                        _ => return Err("readFailed".into()),
                    })
                }
                rows.push(row)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_includes_committed_wal_and_remains_consistent_after_source_changes() {
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
        };
        let root = std::env::temp_dir().join(format!("ot3-cookie-wal-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("cookies.sqlite");
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let script="import sqlite3,sys\ndb=sqlite3.connect(sys.argv[1])\ndb.execute('pragma journal_mode=WAL')\ndb.execute('create table cookies(value)')\ndb.execute(\"insert into cookies values('first')\")\ndb.commit()\nprint('ready',flush=True)\nsys.stdin.read()\n";
        let mut child = Child(
            Command::new("python3")
                .arg("-c")
                .arg(script)
                .arg(&path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut ready = String::new();
        BufReader::new(child.0.stdout.take().unwrap())
            .read_line(&mut ready)
            .unwrap();
        assert_eq!(ready.trim(), "ready");
        assert!(root.join("cookies.sqlite-wal").is_file());
        let snapshot = Database::snapshot(&path).unwrap();
        assert_eq!(
            snapshot.rows("select count(*) from cookies").unwrap()[0][0]
                .number()
                .unwrap(),
            1.
        );
        assert!(Command::new("sqlite3")
            .arg(&path)
            .arg("insert into cookies values('second');")
            .status()
            .unwrap()
            .success());
        assert_eq!(
            snapshot.rows("select count(*) from cookies").unwrap()[0][0]
                .number()
                .unwrap(),
            1.,
            "existing snapshot must not change when source commits"
        );
        let fresh = Database::snapshot(&path).unwrap();
        assert_eq!(
            fresh.rows("select count(*) from cookies").unwrap()[0][0]
                .number()
                .unwrap(),
            2.,
            "new snapshot sees committed WAL rows"
        );
        drop(child);
        std::fs::remove_dir_all(root).unwrap();
    }
}
