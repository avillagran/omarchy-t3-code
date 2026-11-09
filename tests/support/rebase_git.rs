//! Native fixture Git except explicit rebase finalization. That command boundary
//! retains state directories and models ref/index/file recovery without teardown.
use omarchy_t3_client::{journal_vault as v,Result};
use super::retaining_fs::RetainingFs;
use std::{collections::BTreeMap,fs,path::{Path,PathBuf},sync::Mutex};
pub struct RetainingRebaseGit {
    path:PathBuf,head:String,branch:String,files:BTreeMap<String,Vec<u8>>,
    deletion:RetainingFs,requests:Mutex<Vec<Vec<String>>>,
}
impl RetainingRebaseGit {
    pub fn new(path:&Path)->Self{
        let head=v::run_git(Some(path),&["rev-parse","HEAD"],None,10).stdout;
        let branch=v::run_git(Some(path),&["symbolic-ref","-q","HEAD"],None,10).stdout;
        let listed=v::run_git(Some(path),&["ls-files","-z"],None,10);
        assert!(listed.ok);
        let files=listed.stdout.split('\0').filter(|name|!name.is_empty()).map(|name|{
            let file=path.join(name);assert!(!fs::symlink_metadata(&file).unwrap().file_type().is_symlink());
            (name.to_owned(),fs::read(file).unwrap())
        }).collect();
        Self{path:path.into(),head,branch,files,deletion:RetainingFs::new(path.parent().unwrap().into()),requests:Mutex::new(Vec::new())}
    }
    fn git(&self,args:&[&str])->v::GitOutput{v::run_git(Some(&self.path),args,None,120)}
    fn keep_metadata(&self){
        for name in ["rebase-merge","rebase-apply"]{
            let path=self.path.join(".git").join(name);
            if path.exists(){self.deletion.remove(&path).unwrap();}
        }
        for name in ["REBASE_HEAD","MERGE_MSG","AUTO_MERGE","CHERRY_PICK_HEAD"]{
            match fs::remove_file(self.path.join(".git").join(name)){Ok(())=>{},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},Err(e)=>panic!("{e}")}
        }
    }
    fn select_head(&self,head:&str){
        if self.branch.is_empty(){assert!(self.git(&["update-ref","HEAD",head]).ok)}
        else{assert!(self.git(&["update-ref",&self.branch,head]).ok);assert!(self.git(&["symbolic-ref","HEAD",&self.branch]).ok)}
    }
    pub fn requests(&self)->Vec<Vec<String>>{self.requests.lock().unwrap().clone()}
    pub fn retained(&self)->Vec<(PathBuf,PathBuf)>{self.deletion.receipts()}
}
impl v::RebaseIo for RetainingRebaseGit {
    fn rebasing(&self)->bool{["rebase-merge","rebase-apply"].iter().any(|name|self.path.join(".git").join(name).is_dir())}
    fn run(&self,args:&[&str],seconds:u32)->v::GitOutput{
        self.requests.lock().unwrap().push(args.iter().map(|a|(*a).to_owned()).collect());
        if args==["rebase","--abort"]{
            self.keep_metadata();assert!(self.git(&["read-tree","--reset",&self.head]).ok);
            for(name,bytes)in &self.files{let path=self.path.join(name);fs::create_dir_all(path.parent().unwrap()).unwrap();fs::write(path,bytes).unwrap();}
            self.select_head(&self.head);
            return v::GitOutput{ok:true,stdout:String::new(),stderr:String::new()}
        }
        if args.contains(&"rebase")&&args.contains(&"--continue"){
            let todo=self.path.join(".git/rebase-merge/git-rebase-todo");
            assert!(fs::read_to_string(todo).unwrap_or_default().lines().all(|line|line.trim().is_empty()||line.starts_with('#')),"fixture models its one remaining commit");
            let message=fs::read_to_string(self.path.join(".git/rebase-merge/message")).unwrap_or_else(|_|self.git(&["log","-1","--format=%B",&self.head]).stdout);
            // Retire state before ordinary commit; Git never receives a rebase
            // cleanup command and cannot traverse that retained state directory.
            self.keep_metadata();
            assert!(!self.path.join(".git/sequencer").exists());
            let committed=self.git(&["-c","core.editor=true","commit","-m",&message]);assert!(committed.ok,"{}",committed.stderr);
            let head=self.git(&["rev-parse","HEAD"]).stdout;self.select_head(&head);
            return committed
        }
        v::run_git(Some(&self.path),args,None,seconds)
    }
    fn replace_managed(&self,file:&str,contents:&str)->Result<()>{
        if Path::new(file).components().any(|c|!matches!(c,std::path::Component::Normal(_))){return Err("unsafe fixture file".into())}
        fs::write(self.path.join(file),contents).map_err(|e|e.to_string())
    }
}
