//! In-memory regular-file PAX archives for the host-sync GNU tar consumer.
//! Extended records use POSIX byte lengths, including the final newline:
//! https://www.gnu.org/s/tar/manual/html_chapter/Formats.html
use std::{io,path::{Component,Path},time::{SystemTime,UNIX_EPOCH}};

pub(crate) struct Archive { bytes:Vec<u8>, next:u64, modified:u64 }
impl Archive {
    pub(crate) fn new()->Self{Self{bytes:Vec::new(),next:0,modified:SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()}}
    pub(crate) fn file(&mut self,path:&str,contents:&[u8],mode:u32)->io::Result<()> {
        self.entry(path,contents,mode,b'0')
    }
    pub(crate) fn directory(&mut self,path:&str)->io::Result<()> {self.entry(&format!("{}/",path.trim_end_matches('/')),&[],0o700,b'5')}
    fn entry(&mut self,path:&str,contents:&[u8],mode:u32,kind:u8)->io::Result<()> {
        if path.is_empty()||path.contains('\0')||Path::new(path).components().any(|c|!matches!(c,Component::Normal(_))){return Err(io::Error::other("invalid archive path"))}
        let mut size=path.len()+8;
        let extended=loop{let line=format!("{size} path={path}\n");if line.len()==size{break line}size=line.len();};
        let identity=self.next;self.next+=1;
        self.header(&format!("PaxHeaders/entry-{identity}"),extended.len() as u64,0o600,b'x')?;
        self.data(extended.as_bytes());
        self.header(&format!("entry-{identity}"),contents.len() as u64,mode,kind)?;
        self.data(contents);
        Ok(())
    }
    fn header(&mut self,name:&str,size:u64,mode:u32,kind:u8)->io::Result<()> {
        let mut header=[0u8;512];header[..name.len()].copy_from_slice(name.as_bytes());
        octal(&mut header[100..108],u64::from(mode&0o7777))?;
        octal(&mut header[108..116],u64::from(unsafe{libc::geteuid()}))?;
        octal(&mut header[116..124],u64::from(unsafe{libc::getegid()}))?;
        octal(&mut header[124..136],size)?;octal(&mut header[136..148],self.modified)?;
        header[148..156].fill(b' ');header[156]=kind;
        header[257..263].copy_from_slice(b"ustar\0");header[263..265].copy_from_slice(b"00");
        let checksum:u32=header.iter().map(|v|u32::from(*v)).sum();
        header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
        self.bytes.extend_from_slice(&header);Ok(())
    }
    fn data(&mut self,data:&[u8]){self.bytes.extend_from_slice(data);let padding=(512-data.len()%512)%512;self.bytes.resize(self.bytes.len()+padding,0);}
    pub(crate) fn finish(mut self)->Vec<u8>{self.bytes.resize(self.bytes.len()+1024,0);self.bytes}
}
fn octal(field:&mut[u8],value:u64)->io::Result<()> {
    let text=format!("{value:o}");
    if text.len()>=field.len(){return Err(io::Error::other("archive numeric field overflow"))}
    field.fill(b'0');let end=field.len()-1;field[end]=0;field[end-text.len()..end].copy_from_slice(text.as_bytes());Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write,process::{Command,Stdio}};
    fn tar(input:&[u8],args:&[&str])->Vec<u8>{
        let mut child=Command::new("tar").args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output=child.wait_with_output().unwrap();
        assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));output.stdout
    }
    #[test]
    fn actual_gnu_tar_reads_binary_long_unicode_paths_and_modes(){
        let mut archive=Archive::new();
        archive.file("stamp",b"20261106T000000Z",0o600).unwrap();
        archive.file("plan",b"synthetic-plan",0o600).unwrap();
        archive.directory("files").unwrap();
        let path=format!("files/{}/日本🙂.json","long".repeat(50));
        let payload=b"\0binary\xff\n";
        archive.file(&path,payload,0o640).unwrap();
        let bytes=archive.finish();
        assert_eq!(tar(&bytes,&["-xOf","-","--",&path]),payload);
        let names=String::from_utf8(tar(&bytes,&["-tf","-"])).unwrap();
        assert_eq!(names,format!("stamp\nplan\nfiles/\n{path}\n"));
        let verbose=String::from_utf8(tar(&bytes,&["-tvf","-","--numeric-owner"])).unwrap();
        assert!(verbose.lines().any(|line|line.starts_with("-rw-r-----")&&line.contains(&path)));
    }
    #[test]
    fn unsafe_paths_are_rejected_without_polluting_a_later_valid_archive(){
        let mut archive=Archive::new();
        for path in ["","/absolute","../outside","files/../../outside","nul\0name"]{assert!(archive.file(path,b"no",0o600).is_err());}
        archive.file("owned",b"yes",0o600).unwrap();
        let bytes=archive.finish();assert_eq!(tar(&bytes,&["-tf","-"]),b"owned\n");
    }
}
