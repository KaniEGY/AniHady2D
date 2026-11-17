//! Optional Google Drive backups via the official Google OAuth desktop flow.
//! No Google passwords, embedded client secrets or unencrypted refresh tokens.
//! Backups live in Drive's private appDataFolder, not in the user's visible root.
//! A Google Cloud *Desktop* OAuth client ID must be provided for public releases.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io::{Read, Write},net::TcpListener,path::{Path,PathBuf},
    sync::mpsc, thread, time::{Duration,Instant,SystemTime,UNIX_EPOCH}};

const SERVICE: &str = "StickStudio.GoogleDrive";
const SCOPES: &str = "openid email https://www.googleapis.com/auth/drive.appdata";
const MAX_PROJECT_BYTES: u64 = 80 * 1024 * 1024;

#[derive(Debug,Clone)]
pub struct CloudFile {
    pub id: String,
    pub name: String,
    pub modified: String,
}
#[derive(Debug,Clone)]
pub struct Identity {
    pub email: String,
    pub account_id: String,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct CloudProfile {
    pub username: String,
    pub xp: u64,
    pub total_frames_drawn: u64,
    pub total_seconds_spent: u64,
    pub total_painted_layers: u64,
}
impl CloudProfile {
    pub fn from_user(p:&crate::profile::UserProfile)->Self {
        Self { username:p.username.clone(),xp:p.xp,
            total_frames_drawn:p.total_frames_drawn,
            total_seconds_spent:p.total_seconds_spent,
            total_painted_layers:p.total_painted_layers }
    }
    pub fn apply(&self,p:&mut crate::profile::UserProfile) {
        // Never take over tokens, cloud account or device-specific avatar paths.
        p.username=self.username.clone();
        p.xp=self.xp;
        p.total_frames_drawn=self.total_frames_drawn;
        p.total_seconds_spent=self.total_seconds_spent;
        p.total_painted_layers=self.total_painted_layers;
        p.save();
    }
}
#[derive(Debug)]
pub enum Event {
    SignedIn(Identity),
    BackedUp(usize),
    Listed(Vec<CloudFile>),
    RestoredProject(PathBuf),
    RestoredProfile(CloudProfile),
    Error(String),
}

fn random_bytes<const N: usize>() -> Result<[u8;N],String> {
    let mut bytes=[0;N];
    getrandom::getrandom(&mut bytes).map_err(|e|e.to_string())?;
    Ok(bytes)
}
fn encode(s:&str)->String {
    let mut result=String::new();
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_'|b'.'|b'~') {
            result.push(b as char);
        } else {
            result.push_str(&format!("%{:02X}",b));
        }
    }
    result
}
fn decode(s:&str)->String {
    let bytes=s.as_bytes();let mut out=Vec::new();let mut i=0;
    while i<bytes.len() {
        if bytes[i]==b'%' && i+2<bytes.len() {
            if let (Some(a),Some(b))=((bytes[i+1] as char).to_digit(16),
                (bytes[i+2] as char).to_digit(16)) {
                out.push((a*16+b) as u8);i+=3;continue;
            }
        }
        out.push(if bytes[i]==b'+' {b' '} else {bytes[i]});i+=1;
    }
    String::from_utf8_lossy(&out).to_string()
}
fn parameter<'a>(s:&'a str,key:&str)->Option<String> {
    s.split('&').filter_map(|part|part.split_once('='))
        .find(|(k,_)|*k==key).map(|(_,v)|decode(v))
}
fn browser_open(url:&str)->Result<(),String> {
    #[cfg(windows)]
    let child=std::process::Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler").arg(url).spawn();
    #[cfg(target_os="macos")]
    let child=std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(windows,target_os="macos")))]
    let child=std::process::Command::new("xdg-open").arg(url).spawn();
    child.map(|_|()).map_err(|e|format!("Could not open browser: {e}"))
}
fn http_json(res:Result<ureq::Response,ureq::Error>)->Result<Value,String>{
    match res {
        Ok(r)=>r.into_json::<Value>().map_err(|e|format!("Invalid Google response: {e}")),
        Err(ureq::Error::Status(status,r))=>{
            let details=r.into_string().unwrap_or_default();
            let clipped: String=details.chars().take(350).collect();
            Err(format!("Google HTTP {status}: {clipped}"))
        }
        Err(e)=>Err(format!("Google network error: {e}"))
    }
}
fn client_id_check(client_id:&str)->Result<(),String>{
    if client_id.len()>300 || !client_id.ends_with(".apps.googleusercontent.com")
        || !client_id.bytes().all(|c|c.is_ascii_alphanumeric()||matches!(c,b'-'|b'_'|b'.')) {
        return Err("Google Desktop OAuth Client ID is not configured. Create one in Google Cloud and set STICK_GOOGLE_CLIENT_ID for the release.".to_string());
    }
    Ok(())
}
fn credential_entry(account_id:&str)->Result<keyring::Entry,String>{
    keyring::Entry::new(SERVICE,account_id).map_err(|e|e.to_string())
}
pub fn sign_out(account_id:&str)->Result<(),String>{
    if account_id.is_empty(){return Ok(());}
    let entry=credential_entry(account_id)?;
    match entry.delete_credential(){
        Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),
        Err(e)=>Err(e.to_string()),
    }
}
pub fn sign_in(client_id:&str)->Result<Identity,String> {
    client_id_check(client_id)?;
    let listener=TcpListener::bind("127.0.0.1:0")
        .map_err(|e|format!("Cannot bind OAuth callback: {e}"))?;
    listener.set_nonblocking(true).map_err(|e|e.to_string())?;
    let port=listener.local_addr().map_err(|e|e.to_string())?.port();
    let redirect=format!("http://127.0.0.1:{port}/oauth2callback");
    let verifier=URL_SAFE_NO_PAD.encode(random_bytes::<64>()?);
    let challenge=URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state=URL_SAFE_NO_PAD.encode(random_bytes::<24>()?);
    let query=[
        ("client_id",client_id),("redirect_uri",&redirect),
        ("response_type","code"),("scope",SCOPES),
        ("code_challenge",&challenge),("code_challenge_method","S256"),
        ("state",&state),("access_type","offline"),("prompt","consent"),
    ];
    let params=query.iter().map(|(key,value)|format!("{key}={}",encode(value)))
        .collect::<Vec<_>>().join("&");
    browser_open(&format!("https://accounts.google.com/o/oauth2/v2/auth?{params}"))?;
    let deadline=Instant::now()+Duration::from_secs(120);
    let code=loop {
        if Instant::now()>=deadline {return Err("Google sign-in timed out. Try again.".into());}
        match listener.accept() {
            Ok((mut stream,_))=>{
                stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
                stream.set_write_timeout(Some(Duration::from_secs(3))).ok();
                let mut data=[0u8;8192];
                let n=stream.read(&mut data).map_err(|e|e.to_string())?;
                let req=String::from_utf8_lossy(&data[..n]);
                let target=req.split_whitespace().nth(1).unwrap_or("");
                if !target.starts_with("/oauth2callback?") {continue;}
                let query=target.split_once('?').map(|(_,q)|q).unwrap_or("");
                let secure=parameter(query,"state").as_deref()==Some(&state);
                let code=if secure {parameter(query,"code")}else{None};
                let response=if code.is_some() {"Sign-in completed. You can return to Stick Studio."}
                    else {"Sign-in was not accepted. Return to Stick and retry."};
                let body=format!("<!doctype html><meta charset='utf-8'><title>Stick Studio</title><p>{response}</p>");
                let message=format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                let _=stream.write_all(message.as_bytes());
                if let Some(code)=code {break code;}
                if let Some(err)=parameter(query,"error") {
                    return Err(format!("Google authorization denied: {err}"));
                }
            }
            Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>
                thread::sleep(Duration::from_millis(100)),
            Err(e)=>return Err(e.to_string())
        }
    };
    let response=http_json(ureq::post("https://oauth2.googleapis.com/token")
        .timeout(Duration::from_secs(20))
        .send_form(&[
            ("code",code.as_str()),("client_id",client_id),
            ("redirect_uri",&redirect),("code_verifier",&verifier),
            ("grant_type","authorization_code")
        ]))?;
    let token=response["access_token"].as_str().ok_or("Missing access token")?;
    let refresh=response["refresh_token"].as_str().ok_or(
        "Google did not provide a refresh token. Revoke the previous grant and retry.")?;
    let info=http_json(ureq::get("https://openidconnect.googleapis.com/v1/userinfo")
        .set("Authorization",&format!("Bearer {token}"))
        .timeout(Duration::from_secs(20)).call())?;
    let account_id=info["sub"].as_str().filter(|s|!s.is_empty()).ok_or("Google account id missing")?.to_owned();
    let email=info["email"].as_str().filter(|s|!s.is_empty()).ok_or("Google email missing")?.to_owned();
    credential_entry(&account_id)?.set_password(refresh)
        .map_err(|e|format!("Could not save refresh token to Windows Credential Manager: {e}"))?;
    Ok(Identity{email,account_id})
}
fn access_token(client_id:&str,account_id:&str)->Result<String,String> {
    client_id_check(client_id)?;
    let refresh=credential_entry(account_id)?.get_password()
        .map_err(|_|"No saved Google authorization. Sign in again.".to_string())?;
    let response=http_json(ureq::post("https://oauth2.googleapis.com/token")
        .timeout(Duration::from_secs(20))
        .send_form(&[
            ("client_id",client_id),("refresh_token",&refresh),
            ("grant_type","refresh_token")
        ]))?;
    response["access_token"].as_str().map(str::to_owned)
        .ok_or_else(||"Google refresh failed. Sign in again.".to_string())
}
fn list(token:&str)->Result<Vec<CloudFile>,String>{
    let mut found=Vec::new();
    let mut page=String::new();
    loop{
        let mut req=ureq::get("https://www.googleapis.com/drive/v3/files")
            .set("Authorization",&format!("Bearer {token}"))
            .query("spaces","appDataFolder")
            .query("fields","nextPageToken,files(id,name,modifiedTime,size)")
            .query("pageSize","1000")
            .timeout(Duration::from_secs(25));
        if !page.is_empty(){req=req.query("pageToken",&page);}
        let response=http_json(req.call())?;
        if let Some(items)=response["files"].as_array(){
            for item in items {
                let (Some(id),Some(name))=(item["id"].as_str(),item["name"].as_str())
                    else{continue};
                if name=="StickProfile.json" || name.starts_with("StickProject_"){
                    found.push(CloudFile{
                        id:id.to_owned(), name:name.to_owned(),
                        modified:item["modifiedTime"].as_str().unwrap_or("").to_owned(),
                    });
                }
            }
        }
        page=response["nextPageToken"].as_str().unwrap_or("").to_string();
        if page.is_empty() {break;}
        if found.len()>5000{return Err("Too many cloud items.".into());}
    }
    found.sort_by(|a,b|b.modified.cmp(&a.modified));
    Ok(found)
}
fn upload(token:&str,name:&str,bytes:&[u8],existing:&[CloudFile])->Result<(),String>{
    if bytes.len() as u64>MAX_PROJECT_BYTES {
        return Err(format!("{name}: project is larger than the 80 MB backup limit."));
    }
    if let Some(id)=existing.iter().find(|f|f.name==name).map(|f|&f.id){
        let address=format!("https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media",encode(id));
        http_json(ureq::patch(&address).set("Authorization",&format!("Bearer {token}"))
            .set("Content-Type","application/octet-stream")
            .timeout(Duration::from_secs(90)).send_bytes(bytes))?;
    }else{
        let boundary=format!("stick_{}",URL_SAFE_NO_PAD.encode(random_bytes::<20>()?));
        let metadata=json!({"name":name,"parents":["appDataFolder"]});
        let mut body=format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{}\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n",metadata).into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        http_json(ureq::post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart")
            .set("Authorization",&format!("Bearer {token}"))
            .set("Content-Type",&format!("multipart/related; boundary={boundary}"))
            .timeout(Duration::from_secs(90)).send_bytes(&body))?;
    }
    Ok(())
}
fn project_cloud_name(path:&Path)->String{
    let hash=Sha256::digest(path.to_string_lossy().as_bytes());
    let prefix=hash[..7].iter().map(|b|format!("{:02x}",b)).collect::<String>();
    let label=path.file_name().and_then(|x|x.to_str()).unwrap_or("project.stick");
    let label:String=label.chars().map(|c|if c.is_ascii_alphanumeric()||c=='.'||c=='_'||c=='-' {c}else{'_'}).take(85).collect();
    format!("StickProject_{prefix}_{label}")
}
pub fn backup(client_id:&str,account_id:&str,profile:CloudProfile,paths:Vec<PathBuf>)
    ->Result<usize,String>{
    let token=access_token(client_id,account_id)?;
    let previous=list(&token)?;
    let mut successes=0;
    upload(&token,"StickProfile.json",&serde_json::to_vec(&profile).map_err(|e|e.to_string())?,&previous)?;
    successes+=1;
    for path in paths.into_iter().take(100) {
        if !path.is_file(){continue;}
        let size=path.metadata().map_err(|e|e.to_string())?.len();
        if size>MAX_PROJECT_BYTES {continue;}
        let data=std::fs::read(&path).map_err(|e|e.to_string())?;
        upload(&token,&project_cloud_name(&path),&data,&previous)?;
        successes+=1;
    }
    Ok(successes)
}
pub fn list_cloud(client_id:&str,account_id:&str)->Result<Vec<CloudFile>,String>{
    let token=access_token(client_id,account_id)?;
    list(&token)
}
pub fn restore(client_id:&str,account_id:&str,file:CloudFile)->Result<Event,String>{
    if !file.id.bytes().all(|c|c.is_ascii_alphanumeric()||matches!(c,b'-'|b'_')) {
        return Err("Invalid cloud file id".into());
    }
    if file.name!="StickProfile.json" && !file.name.starts_with("StickProject_") {
        return Err("Not a Stick Studio backup.".into());
    }
    let token=access_token(client_id,account_id)?;
    let address=format!("https://www.googleapis.com/drive/v3/files/{}?alt=media",file.id);
    let response=ureq::get(&address)
        .set("Authorization",&format!("Bearer {token}"))
        .timeout(Duration::from_secs(90)).call().map_err(|e|e.to_string())?;
    if response.header("Content-Length").and_then(|s|s.parse::<u64>().ok())
        .is_some_and(|size|size>MAX_PROJECT_BYTES) {
        return Err("Cloud backup exceeds maximum allowed size.".into());
    }
    let mut bytes=Vec::new();
    response.into_reader().take(MAX_PROJECT_BYTES+1).read_to_end(&mut bytes)
        .map_err(|e|e.to_string())?;
    if bytes.len() as u64>MAX_PROJECT_BYTES {return Err("Cloud backup too large.".into());}
    if file.name=="StickProfile.json" {
        let profile=serde_json::from_slice::<CloudProfile>(&bytes)
            .map_err(|e|format!("Invalid backup profile: {e}"))?;
        return Ok(Event::RestoredProfile(profile));
    }
    // Never overwrite active projects or arbitrary file paths from Drive.
    let filename:String=file.name.chars()
        .filter(|c|c.is_ascii_alphanumeric()||matches!(c,'.'|'_'|'-'))
        .take(120).collect();
    if filename.is_empty(){return Err("Invalid backup filename.".into());}
    let base=std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir).join("StickStudio").join("CloudRestores");
    std::fs::create_dir_all(&base).map_err(|e|e.to_string())?;
    let stamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let target=base.join(format!("{stamp}_{filename}"));
    std::fs::write(&target,bytes).map_err(|e|e.to_string())?;
    Ok(Event::RestoredProject(target))
}
pub fn spawn_job<F>(work:F)->mpsc::Receiver<Event> where F:FnOnce()->Result<Event,String>+Send+'static {
    let (tx,rx)=mpsc::channel();
    thread::spawn(move||{
        let event=work().unwrap_or_else(Event::Error);
        let _=tx.send(event);
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_is_urlsafe_and_query_is_encoded() {
        assert_eq!(decode("a%2Bb%20c"),"a+b c");
        assert_eq!(parameter("state=a%2Fb&code=123","state"),Some("a/b".to_string()));
        assert_eq!(encode("openid email"),"openid%20email");
        assert!(client_id_check("1234-abc.apps.googleusercontent.com").is_ok());
        assert!(client_id_check("malicious;command").is_err());
    }
    #[test]
    fn cloud_names_never_include_local_parent_paths(){
        let n=project_cloud_name(Path::new("C:/private/work/smile.stick"));
        assert!(n.starts_with("StickProject_"));
        assert!(n.ends_with("_smile.stick"));
        assert!(!n.contains("private"));
    }
}
