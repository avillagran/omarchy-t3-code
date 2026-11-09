use omarchy_t3_client::frame_files;
use std::{fs,os::unix::fs::symlink};
fn fixture()->std::path::PathBuf{let path=std::env::temp_dir().join(format!("owned-frame-files-{}",uuid::Uuid::new_v4()));fs::create_dir_all(&path).unwrap();path}
#[test]
fn fixed_slots_are_reclaimed_without_touching_unknown_entries(){
    let root=fixture();let frames=root.join("frames");fs::create_dir(&frames).unwrap();
    for file in ["frame-0.png","frame-1.bmp","frame-0.jpg.next"]{fs::write(frames.join(file),b"frame").unwrap();}
    fs::write(frames.join("keep"),b"unowned").unwrap();
    assert!(frame_files::reclaim(&frames).is_err());
    for file in ["frame-0.png","frame-1.bmp","frame-0.jpg.next"]{assert!(!frames.join(file).exists());}
    assert_eq!(fs::read(frames.join("keep")).unwrap(),b"unowned");
}
#[test]
fn root_links_are_refused_and_slot_links_unlinked_without_following(){
    let root=fixture();let frames=root.join("frames");fs::create_dir(&frames).unwrap();let target=root.join("retained");fs::write(&target,b"keep").unwrap();
    symlink(&target,frames.join("frame-0.png")).unwrap();
    let linked=root.join("linked");symlink(&frames,&linked).unwrap();
    assert!(frame_files::reclaim(&linked).is_err());assert!(fs::symlink_metadata(frames.join("frame-0.png")).is_ok());
    frame_files::reclaim(&frames).unwrap();assert_eq!(fs::read(&target).unwrap(),b"keep");
}
#[test]
fn a_directory_named_as_a_slot_is_never_traversed(){
    let root=fixture();let frames=root.join("frames");fs::create_dir_all(frames.join("frame-0.png")).unwrap();
    fs::write(frames.join("frame-0.png/keep"),b"keep").unwrap();
    assert!(frame_files::reclaim(&frames).is_err());assert_eq!(fs::read(frames.join("frame-0.png/keep")).unwrap(),b"keep");
}
