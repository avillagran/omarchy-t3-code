//! QML components written by the agent, previewed in a locked-down Qt process.
//! Layers: an import allowlist checked here, a renderer engine that only sees
//! the allowed Qt modules and refuses network and outside URLs, a scrubbed
//! environment and a Landlock domain (no file reads outside Qt and fonts, no
//! TCP, no program launches).
use crate::*;
use std::ffi::CString;

/// Modules a preview may import; everything else is refused before Qt starts.
pub const ALLOWED_IMPORTS:[&str;6]=["QtQuick","QtQuick.Controls","QtQuick.Layouts","QtQuick.Shapes","QtQuick.Effects","QtQml"];
const LIMIT:usize=64*1024;
/// Data segment limit for the renderer (RLIMIT_DATA).
pub const MEMORY_LIMIT:u64=512*1024*1024;

/// Comments removed, string literals kept: imports inside strings (for
/// Qt.createQmlObject) are checked too.
fn strip_comments(source:&str)->String{
 let b=source.as_bytes();let mut out=Vec::with_capacity(b.len());let mut i=0;let mut quote=0u8;
 while i<b.len(){
  let c=b[i];
  if quote!=0{out.push(c);if c==b'\\'&&i+1<b.len(){out.push(b[i+1]);i+=2;continue}if c==quote{quote=0}i+=1;continue}
  if c==b'"'||c==b'\''||c==b'`'{quote=c;out.push(c);i+=1;continue}
  if c==b'/'&&b.get(i+1)==Some(&b'/'){while i<b.len()&&b[i]!=b'\n'{i+=1}continue}
  if c==b'/'&&b.get(i+1)==Some(&b'*'){i+=2;while i<b.len()&&!(b[i]==b'*'&&b.get(i+1)==Some(&b'/')){if b[i]==b'\n'{out.push(b'\n')}i+=1}i+=2;out.push(b' ');continue}
  out.push(c);i+=1;
 }
 String::from_utf8_lossy(&out).into_owned()
}
/// Every `import X` statement in the source, as written (module or quoted path).
pub fn imports(source:&str)->Vec<String>{
 let code=strip_comments(source);let b=code.as_bytes();let mut found=vec![];let mut from=0;
 while let Some(at)=code[from..].find("import").map(|p|p+from){
  from=at+6;
  if at>0&&(b[at-1].is_ascii_alphanumeric()||matches!(b[at-1],b'_'|b'.'|b'$')){continue}
  let mut i=at+6;if !b.get(i).is_some_and(|c|*c==b' '||*c==b'\t'){continue}
  while b.get(i).is_some_and(|c|*c==b' '||*c==b'\t'){i+=1}
  match b.get(i){
   // A quoted import pulls a file or directory: never allowed.
   Some(b'"')|Some(b'\'')=>{let q=b[i];let end=code[i+1..].find(q as char).map(|e|i+1+e).unwrap_or(b.len());found.push(code[i..(end+1).min(b.len())].to_string());}
   Some(c) if c.is_ascii_alphabetic()||*c==b'_'=>{
    let start=i;while b.get(i).is_some_and(|c|c.is_ascii_alphanumeric()||matches!(c,b'_'|b'.')){i+=1}
    let module=code[start..i].trim_end_matches('.').to_string();
    let mut j=i;while b.get(j).is_some_and(|c|*c==b' '||*c==b'\t'){j+=1}
    if b.get(j).is_some_and(|c|c.is_ascii_digit()){while b.get(j).is_some_and(|c|c.is_ascii_digit()||*c==b'.'){j+=1}while b.get(j).is_some_and(|c|*c==b' '||*c==b'\t'){j+=1}}
    // Only statement-shaped matches count, so prose such as "import data" in a label is left alone.
    let rest=&code[j..];
    if rest.is_empty()||rest.starts_with(['\n','\r',';'])||rest.starts_with("\\n")||rest.starts_with("\\r")||rest.starts_with("as ")||rest.starts_with("as\t"){found.push(module)}
   }
   _=>{}
  }
 }
 found
}
/// The preview source if every import is allowed; otherwise an error code the
/// panel explains (`qml_import_blocked:<module>`).
pub fn validate(source:&str)->Result<()>{
 if source.trim().is_empty(){return Err("qml_preview_empty".into())}
 if source.len()>LIMIT{return Err("qml_preview_too_large".into())}
 if source.contains('\0'){return Err("qml_preview_invalid".into())}
 for module in imports(source){if !ALLOWED_IMPORTS.contains(&module.as_str()){return Err(format!("qml_import_blocked:{}",module.chars().take(120).collect::<String>()))}}
 Ok(())
}
/// The document the offscreen renderer opens in QML mode.
pub fn page(input:&Value)->Result<Value>{
 let source=input["attachment"]["source"].as_str().ok_or("qml_preview_empty")?;
 validate(source)?;
 Ok(json!({"generated":true,"qmlPreview":true,"contents":source,"html":true,"markdown":false,"internalReview":false,"title":input["attachment"]["name"].as_str().filter(|n|!n.is_empty()).unwrap_or("QML"),"labels":input["attachment"]["labels"],"theme":input["theme"]}))
}

/// The renderer process for a preview: nothing inherited (tokens, D-Bus,
/// display, import paths), no PATH to launch helpers from, and the Landlock
/// domain entered before exec.
pub fn command(exe:&std::path::Path,frames:&std::path::Path)->std::process::Command{
 use std::os::unix::process::CommandExt;
 let mut command=std::process::Command::new(exe);command.arg("document-render").process_group(0).env_clear();
 for key in ["HOME","LANG","LC_ALL","LC_CTYPE","XDG_CACHE_HOME"]{if let Some(v)=std::env::var_os(key){command.env(key,v);}}
 command.env("PATH","/nonexistent").env("QML_DISABLE_DISK_CACHE","1").env("QT_QUICK_CONTROLS_STYLE","Fusion").env("QT_QPA_PLATFORM","offscreen").env("QT_QUICK_BACKEND","software").env("QT_SCALE_FACTOR","1").env("QT_SCREEN_SCALE_FACTORS","1");
 let sandbox=Sandbox::renderer(exe,frames);
 // Lower priority than the desktop and a bounded heap (normal previews use under 100 MB).
 unsafe{command.pre_exec(move||{libc::setpriority(libc::PRIO_PROCESS,0,10);let limit=libc::rlimit{rlim_cur:MEMORY_LIMIT,rlim_max:MEMORY_LIMIT};libc::setrlimit(libc::RLIMIT_DATA,&limit);sandbox.enter()});}
 command
}

// Landlock (linux/landlock.h). Missing kernel support leaves the other layers.
const CREATE_RULESET:libc::c_long=444;const ADD_RULE:libc::c_long=445;const RESTRICT_SELF:libc::c_long=446;
const EXECUTE:u64=1;const WRITE_FILE:u64=1<<1;const READ_FILE:u64=1<<2;const READ_DIR:u64=1<<3;const TRUNCATE:u64=1<<14;const IOCTL_DEV:u64=1<<15;
const FILE_ONLY:u64=EXECUTE|WRITE_FILE|READ_FILE|TRUNCATE|IOCTL_DEV;
#[repr(C)]struct RulesetAttr{fs:u64,net:u64,scoped:u64}
#[repr(C,packed)]struct PathBeneath{allowed:u64,parent:i32}
/// Paths the renderer keeps, prepared before fork so the child only makes syscalls.
pub struct Sandbox{rules:Vec<(CString,u64)>}
pub const READ:u64=READ_FILE|READ_DIR;
pub const ALL:u64=u64::MAX;
impl Sandbox{
 /// What the QML renderer needs: Qt and system libraries, fonts, its own plugin files and private frame directory.
 pub fn renderer(exe:&std::path::Path,frames:&std::path::Path)->Self{
  let mut s=Self::new();
  for path in ["/usr","/etc/fonts","/etc/ld.so.cache","/etc/localtime"]{s.allow(path,READ)}
  s.allow("/dev/null",READ_FILE|WRITE_FILE);s.allow("/dev/urandom",READ_FILE);
  if let Ok(home)=std::env::var("HOME"){for rel in [".cache/fontconfig",".config/fontconfig",".local/share/fonts",".fonts"]{s.allow(std::path::Path::new(&home).join(rel),READ)}}
  if let Some(root)=exe.parent().and_then(|bin|bin.parent()){s.allow(root.join("bin"),READ);s.allow(root.join("ui/document"),READ)}
  s.allow(exe,READ_FILE|EXECUTE);s.allow_loader();s.allow(frames,ALL);s
 }
 /// execve also needs the ELF interpreter (ld-linux / ld-musl) to be executable.
 pub fn allow_loader(&mut self){
  for dir in ["/usr/lib","/usr/lib64","/lib","/lib64"]{
   let Ok(entries)=std::fs::read_dir(dir)else{continue};
   for entry in entries.flatten(){let name=entry.file_name();let name=name.to_string_lossy();if name.starts_with("ld-linux")||name.starts_with("ld-musl"){self.allow(entry.path(),READ_FILE|EXECUTE)}}
  }
 }
 pub fn new()->Self{Self{rules:vec![]}}
 pub fn allow(&mut self,path:impl AsRef<std::path::Path>,access:u64){
  use std::os::unix::ffi::OsStrExt;
  if let Ok(path)=CString::new(path.as_ref().as_os_str().as_bytes()){self.rules.push((path,access))}
 }
 /// Enters the domain; called in the child between fork and exec. Kernels
 /// without Landlock are left as they are; a partial domain is an error.
 pub fn enter(&self)->std::io::Result<()>{
  unsafe{
   let abi=libc::syscall(CREATE_RULESET,std::ptr::null::<RulesetAttr>(),0usize,1u32);
   if abi<1{return Ok(())}
   let mut fs:u64=(1<<13)-1;if abi>=2{fs|=1<<13}if abi>=3{fs|=TRUNCATE}if abi>=5{fs|=IOCTL_DEV}
   // ABI 4: no TCP bind/connect; ABI 6: no abstract sockets or signals outside the domain.
   let attr=RulesetAttr{fs,net:if abi>=4{3}else{0},scoped:if abi>=6{3}else{0}};
   let size=if abi>=6{24}else if abi>=4{16}else{8};
   let ruleset=libc::syscall(CREATE_RULESET,&attr as *const RulesetAttr,size as usize,0u32) as i32;
   if ruleset<0{return Err(std::io::Error::last_os_error())}
   for(path,access) in &self.rules{
    let fd=libc::open(path.as_ptr(),libc::O_PATH|libc::O_CLOEXEC);if fd<0{continue}
    let mut st:libc::stat=std::mem::zeroed();let dir=libc::fstat(fd,&mut st)==0&&(st.st_mode&libc::S_IFMT)==libc::S_IFDIR;
    let allowed=access&fs&if dir{u64::MAX}else{FILE_ONLY};
    let rule=PathBeneath{allowed,parent:fd};
    let added=allowed==0||libc::syscall(ADD_RULE,ruleset,1u32,&rule as *const PathBeneath,0u32)==0;
    libc::close(fd);if !added{libc::close(ruleset);return Err(std::io::Error::last_os_error())}
   }
   if libc::prctl(libc::PR_SET_NO_NEW_PRIVS,1,0,0,0)!=0||libc::syscall(RESTRICT_SELF,ruleset,0u32)!=0{let e=std::io::Error::last_os_error();libc::close(ruleset);return Err(e)}
   libc::close(ruleset);Ok(())
  }
 }
}
