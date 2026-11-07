//! Explicit filesystem deletion double: remove a namespace entry by quarantine
//! rename, retain its complete contents, and record the observed mutation.
use std::{fs,io,path::{Component,Path,PathBuf},sync::{Arc,Mutex}};
#[derive(Clone)]
pub struct RetainingFs {root:PathBuf,receipts:Arc<Mutex<Vec<(PathBuf,PathBuf)>>>}
impl RetainingFs {
    pub fn new(root:PathBuf)->Self{Self{root,receipts:Arc::new(Mutex::new(Vec::new()))}}
    pub fn remove(&self,target:&Path)->io::Result<()> {
        let relative=target.strip_prefix(&self.root).map_err(|_|io::Error::other("outside fixture"))?;
        if relative.as_os_str().is_empty(){return Err(io::Error::other("fixture root cannot be removed"))}
        let mut current=self.root.clone();
        for component in relative.components(){
            let Component::Normal(part)=component else{return Err(io::Error::other("unsafe fixture path"))};
            current.push(part);
            if fs::symlink_metadata(&current)?.file_type().is_symlink(){return Err(io::Error::other("fixture deletion cannot follow links"))}
        }
        let retained=self.root.join("retained-delete-requests");fs::create_dir_all(&retained)?;
        let destination=retained.join(uuid::Uuid::new_v4().to_string());
        fs::rename(target,&destination)?;
        self.receipts.lock().unwrap().push((target.to_path_buf(),destination));Ok(())
    }
    pub fn receipts(&self)->Vec<(PathBuf,PathBuf)>{self.receipts.lock().unwrap().clone()}
}
