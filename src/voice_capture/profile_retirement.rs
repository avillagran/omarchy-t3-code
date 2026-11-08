//! A profile lease cannot outlive its unobserved child retirement in reverse.
use super::resources::Profile;
use std::{
    io,
    process::Child,
    sync::{Arc, Mutex},
};

struct Owner<C> {
    child: Mutex<C>,
    _profile: Option<Arc<Profile>>,
}
struct Guard<C>(Option<Arc<Owner<C>>>);
impl<C> Guard<C> {
    fn new(child: C, profile: Option<Arc<Profile>>) -> Self {
        Self(Some(Arc::new(Owner {
            child: Mutex::new(child),
            _profile: profile,
        })))
    }
    fn confirmed(&mut self) {
        drop(self.0.take());
    }
}
impl<C> Drop for Guard<C> {
    fn drop(&mut self) {
        // Spawn rejection, wait failure, panic or an unconfirmed group fence
        // keeps this child and profile quarantined until process termination.
        if let Some(owner) = self.0.take() {
            std::mem::forget(owner);
        }
    }
}
fn start<C, W, S>(
    child: C,
    profile: Option<Arc<Profile>>,
    fenced: bool,
    wait: W,
    spawn: S,
) -> io::Result<()>
where
    C: Send + 'static,
    W: FnOnce(&mut C) -> io::Result<()> + Send + 'static,
    S: FnOnce(Box<dyn FnOnce() + Send>) -> io::Result<()>,
{
    let mut guard = Guard::new(child, profile);
    spawn(Box::new(move || {
        let reaped = match guard.0.as_ref().unwrap().child.lock() {
            Ok(mut child) => wait(&mut child).is_ok(),
            Err(_) => false,
        };
        if reaped && fenced {
            guard.confirmed();
        }
    }))
}
pub(super) fn retire(child: Child, profile: Option<Arc<Profile>>, fenced: bool, name: &str) {
    let _ = start(
        child,
        profile,
        fenced,
        |child| child.wait().map(|_| ()),
        |task| {
            std::thread::Builder::new()
                .name(name.into())
                .spawn(task)
                .map(|_| ())
        },
    );
}
pub(super) fn quarantine(child: Child, profile: Option<Arc<Profile>>) {
    drop(Guard::new(child, profile));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::DirBuilderExt,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct OwnedChild(Arc<AtomicUsize>);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn fixture() -> (PathBuf, Arc<Profile>) {
        assert!(Path::new("/run/.containerenv").exists());
        // Quarantined fixtures stay in container-private RAM until exit. Their
        // retained lock files are never unlinked to make a cleanup test pass.
        let root = Path::new("/dev/shm").join(format!("voice-reaper-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let path = root.join("profile");
        let profile = Arc::new(Profile::open(&path).unwrap());
        (path, profile)
    }
    #[test]
    fn profile_spawn_rejection_quarantines_child_and_real_lock() {
        let (path, profile) = fixture();
        let drops = Arc::new(AtomicUsize::new(0));
        let waits = Arc::new(AtomicUsize::new(0));
        let observed = waits.clone();
        let result = start(
            OwnedChild(drops.clone()),
            Some(profile.clone()),
            true,
            move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
            |task| {
                drop(task);
                Err(io::Error::other("fixture_spawn_rejected"))
            },
        );
        assert!(result.is_err());
        drop(profile);
        assert!(Profile::open(&path).is_err());
        assert_eq!(waits.load(Ordering::SeqCst), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn profile_wait_error_or_missing_fence_quarantines_real_lock() {
        for (fenced, wait_ok) in [(true, false), (false, true)] {
            let (path, profile) = fixture();
            let drops = Arc::new(AtomicUsize::new(0));
            let waits = Arc::new(AtomicUsize::new(0));
            let observed = waits.clone();
            start(
                OwnedChild(drops.clone()),
                Some(profile.clone()),
                fenced,
                move |_| {
                    observed.fetch_add(1, Ordering::SeqCst);
                    if wait_ok {
                        Ok(())
                    } else {
                        Err(io::Error::other("fixture_wait_failed"))
                    }
                },
                |task| {
                    task();
                    Ok(())
                },
            )
            .unwrap();
            drop(profile);
            assert!(Profile::open(&path).is_err());
            assert_eq!(waits.load(Ordering::SeqCst), 1);
            assert_eq!(drops.load(Ordering::SeqCst), 0);
        }
    }
    #[test]
    fn profile_confirmed_reap_and_fence_release_real_lock() {
        let (path, profile) = fixture();
        let drops = Arc::new(AtomicUsize::new(0));
        let waits = Arc::new(AtomicUsize::new(0));
        let observed = waits.clone();
        start(
            OwnedChild(drops.clone()),
            Some(profile.clone()),
            true,
            move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
            |task| {
                task();
                Ok(())
            },
        )
        .unwrap();
        drop(profile);
        let successor = Profile::open(&path).unwrap();
        assert_eq!(waits.load(Ordering::SeqCst), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        drop(successor);
    }
}
