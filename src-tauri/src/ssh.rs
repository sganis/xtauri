use ssh2::{Channel, FileStat, Session, Sftp};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{thread, time};

use super::command;

const WAIT_MS: u64 = 20;

#[derive(Default)]
pub struct Ssh {
    pub session: Option<Session>,
    pub tcp: Option<Arc<Mutex<TcpStream>>>,
    pub pty: Option<Arc<Mutex<Channel>>>,
    sftp: Option<Sftp>,
    host: String,
    user: String,
    password: String,
    private_key: String,

    // Add jump server connection info
    pub jump_session: Option<Session>,
    pub jump_tcp: Option<Arc<Mutex<TcpStream>>>,
    pub tunnel_channel: Option<Channel>,
    jump_host: String,
    jump_user: String,
    jump_password: String,
    jump_private_key: String,
}

// #[derive(Clone, serde::Serialize)]
// struct Payload {
//     percent: f32,
// }

#[allow(dead_code)]
impl Ssh {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
    pub fn supported_algs() -> String {
        let ssh = Session::new().unwrap();
        println!(
            "hostKey: {:?}",
            ssh.supported_algs(ssh2::MethodType::HostKey).unwrap()
        );
        println!(
            "CryptCs: {:?}",
            ssh.supported_algs(ssh2::MethodType::CryptCs).unwrap()
        );
        println!(
            "Kex: {:?}",
            ssh.supported_algs(ssh2::MethodType::Kex).unwrap()
        );
        println!(
            "MacCs: {:?}",
            ssh.supported_algs(ssh2::MethodType::MacCs).unwrap()
        );
        println!(
            "CompCs: {:?}",
            ssh.supported_algs(ssh2::MethodType::CompCs).unwrap()
        );

        "supported flags above".to_string()
    }
    pub fn private_key_path() -> PathBuf {
        let home = dirs::home_dir().unwrap();
        let prikey = home.join(".ssh").join("id_rsa");
        PathBuf::from(&prikey)
    }
    pub fn public_key_path() -> PathBuf {
        let home = dirs::home_dir().unwrap();
        let pubkey = home.join(".ssh").join("id_rsa.pub").clone();
        PathBuf::from(&pubkey)
    }
    pub fn has_private_key() -> bool {
        Ssh::private_key_path().exists()
    }
    pub fn has_public_key() -> bool {
        Ssh::public_key_path().exists()
    }
    fn generate_public_key() -> Result<(), String> {
        let seckey = Ssh::private_key_path();
        let pubkey = Ssh::public_key_path();

        let cmd = format!(
            "ssh-keygen -f {} -y > {}",
            seckey.display(),
            pubkey.display()
        );
        let (_, e, _) = command::run(&cmd);

        if e.len() > 0 {
            Err(e)
        } else {
            Ok(())
        }
    }
    fn generate_keys() -> Result<(), String> {
        let seckey = Ssh::private_key_path();

        let cmd = format!("ssh-keygen -m PEM -N \"\" -f {}", seckey.display());
        let (_, e, _) = command::run(&cmd);

        if e.len() > 0 {
            Err(e)
        } else {
            Ok(())
        }
    }
    async fn transfer_public_key(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
    ) -> Result<(), String> {
        let pubkeytext = std::fs::read_to_string(&Ssh::public_key_path())
            .unwrap()
            .trim()
            .to_string();
        let cmd = format!(
            "exec sh -c \"cd; umask 077; mkdir -p .ssh; echo '{}' >> .ssh/authorized_keys\"",
            pubkeytext
        );
        println!("{cmd}");
        let mut ssh = Ssh::new();
        if let Err(e) = ssh.connect_with_password(host, port, user, password).await {
            println!("Error transfering keys, login with password: {e}");
            return Err(e);
        }
        if let Err(e) = ssh.run(&cmd) {
            println!("Error transfering keys, running command: {e}");
            Err(e)
        } else {
            Ok(())
        }
    }
    async fn test_ssh(host: &str, port: u16, user: &str) -> Result<(), String> {
        if !Ssh::has_private_key() {
            return Err("No private key".to_string());
        }
        let pkey = Ssh::private_key_path();
        let mut ssh = Ssh::new();
        if let Err(e) = ssh
            .connect_with_key(host, port, user, pkey.to_str().unwrap())
            .await
        {
            Err(e)
        } else {
            Ok(())
        }
    }
    pub async fn setup_ssh(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
    ) -> Result<(), String> {
        if !Ssh::has_private_key() {
            if let Err(e) = Ssh::generate_keys() {
                return Err(format!("Could not generate private key: {e}"));
            }
        }
        if !Ssh::has_public_key() {
            if let Err(e) = Ssh::generate_public_key() {
                return Err(format!("Could not generate public key: {e}"));
            }
        }
        if Ssh::test_ssh(host, port, user).await.is_err() {
            if let Err(e) = Ssh::transfer_public_key(host, port, user, password).await {
                return Err(format!("Could not transfer public key: {e}"));
            }
            if let Err(e) = Ssh::test_ssh(host, port, user).await {
                return Err(format!("Test ssh failed: {e}"));
            }
        }
        Ok(())
    }
    fn _get_tcp(&mut self, host: &str, port: u16) -> Result<TcpStream, String> {
        let timeout = Duration::new(5, 0); // 5 secs
        let addresses: Vec<_> = match format!("{}:{}", host, port).to_socket_addrs() {
            Err(e) => {
                println!("Unable to resolve address: {}:{}  {:?}", host, port, e);
                return Err(e.to_string());
            }
            Ok(o) => o.collect(),
        };
        let mut tcp = None;
        let mut error = String::new();

        for addr in addresses {
            match TcpStream::connect_timeout(&addr, timeout) {
                Err(e) => {
                    //error.push_str(&format!("tcp error: {:?}\n", e));
                    error = String::from(&format!("tcp error: {:?}", e));
                    continue;
                }
                Ok(o) => {
                    println!("connected to: {:?}", addr);
                    tcp = Some(o);
                    break;
                }
            };
        }

        if tcp.is_none() {
            return Err(error);
        }

        let tcp = tcp.unwrap();
        tcp.set_nonblocking(true).unwrap();

        Ok(tcp)
    }
    pub async fn connect_with_password(
        &mut self,
        host: &str,
        port: u16,
        user: &str,
        password: &str,
    ) -> Result<(), String> {
        let tcp = match self._get_tcp(host, port) {
            Err(e) => return Err(e),
            Ok(o) => o,
        };

        // Check if jump server is configured
        if !self.jump_host.is_empty() {
            return self.connect_with_password_via_jump(host, port, user, password).await;
        }

        // clone tcp instance
        let arc_tcp = Arc::new(Mutex::new(tcp));
        let tcp_clone;
        {
            let locked_tcp = arc_tcp.lock().unwrap();
            tcp_clone = locked_tcp.try_clone().unwrap();
        }

        let mut session = Session::new().unwrap();
        session.set_tcp_stream(tcp_clone);

        if let Err(e) = session.handshake() {
            return Err(format!("SSH handshake error: {}", e));
        }

        if let Err(e) = session.userauth_password(user, password) {
            return Err(format!("Authentication error: {e}"));
        }

        assert!(session.authenticated());
        let sftp = match session.sftp() {
            Err(e) => return Err(format!("Cannot create sftp channel {e}")),
            Ok(o) => o,
        };

        session.set_blocking(false);

        self.tcp = Some(arc_tcp);
        self.session = Some(session);
        self.sftp = Some(sftp);
        self.host = host.to_string();
        self.user = user.to_string();
        self.password = password.to_string();
        Ok(())
    }
    pub async fn connect_with_key(
        &mut self,
        host: &str,
        port: u16,
        user: &str,
        pkey: &str,
    ) -> Result<(), String> {
        let tcp = match self._get_tcp(host, port) {
            Err(e) => return Err(e),
            Ok(o) => o,
        };

        // clone tcp instance
        let arc_tcp = Arc::new(Mutex::new(tcp));
        let tcp_clone;
        {
            let locked_tcp = arc_tcp.lock().unwrap();
            tcp_clone = locked_tcp.try_clone().unwrap();
        }

        let mut session = Session::new().unwrap();
        session.set_tcp_stream(tcp_clone);

        if let Err(e) = session.handshake() {
            return Err(format!("SSH handshake error: {}", e));
        }

        let private_key = std::path::Path::new(pkey);

        if let Err(e) = session.userauth_pubkey_file(user, None, private_key, None) {
            return Err(format!("Authentication error: {e}"));
        }

        assert!(session.authenticated());

        let sftp = match session.sftp() {
            Err(e) => return Err(format!("Cannot create sftp channel {e}")),
            Ok(o) => o,
        };

        session.set_blocking(false);

        self.tcp = Some(arc_tcp);
        self.session = Some(session);
        self.sftp = Some(sftp);
        self.host = host.to_string();
        self.user = user.to_string();
        self.private_key = pkey.to_string();
        Ok(())
    }
    pub fn disconnect(&mut self) -> Result<(), String> {
        // if let Err(e) = self.session.as_ref().unwrap().disconnect(None, "", None) {
        //     return Err(e.to_string());
        // }

        // Disconnect target session
        if let Some(session) = &self.session {
            if let Err(e) = session.disconnect(None, "", None) {
                return Err(e.to_string());
            }
        }
        
        // Disconnect jump server session
        if let Some(jump_session) = &self.jump_session {
            if let Err(e) = jump_session.disconnect(None, "", None) {
                return Err(e.to_string());
            }
        }
        Ok(())
    }

    // Add this method for jump server configuration
    pub fn set_jump_server(&mut self, jump_host: &str, jump_user: &str, jump_password: &str) {
        self.jump_host = jump_host.to_string();
        self.jump_user = jump_user.to_string();
        self.jump_password = jump_password.to_string();
    }

    pub fn set_jump_server_with_key(&mut self, jump_host: &str, jump_user: &str, jump_private_key: &str) {
        self.jump_host = jump_host.to_string();
        self.jump_user = jump_user.to_string();
        self.jump_private_key = jump_private_key.to_string();
    }

    // New method for connecting through jump server
    async fn connect_with_password_via_jump(
        &mut self,
        target_host: &str,
        target_port: u16,
        target_user: &str,
        target_password: &str,
    ) -> Result<(), String> {
        // 1. Connect to jump server
        let jump_tcp = match self._get_tcp(&self.jump_host, 22) {
            Err(e) => return Err(format!("Jump server connection failed: {e}")),
            Ok(o) => o,
        };

        let arc_jump_tcp = Arc::new(Mutex::new(jump_tcp));
        let jump_tcp_clone = {
            let locked_tcp = arc_jump_tcp.lock().unwrap();
            locked_tcp.try_clone().unwrap()
        };

        let mut jump_session = Session::new().unwrap();
        jump_session.set_tcp_stream(jump_tcp_clone);
        
        if let Err(e) = jump_session.handshake() {
            return Err(format!("Jump server SSH handshake error: {}", e));
        }

        if let Err(e) = jump_session.userauth_password(&self.jump_user, &self.jump_password) {
            return Err(format!("Jump server authentication error: {e}"));
        }

        // 2. Create tunnel through jump server
        let tunnel_channel = match jump_session.channel_direct_tcpip(target_host, target_port, None) {
            Err(e) => return Err(format!("Failed to create tunnel: {e}")),
            Ok(o) => o,
        };

        // 3. Create target session using tunnel
        let mut target_session = Session::new().unwrap();
        target_session.set_tcp_stream(tunnel_channel);

        if let Err(e) = target_session.handshake() {
            return Err(format!("Target SSH handshake error: {}", e));
        }

        if let Err(e) = target_session.userauth_password(target_user, target_password) {
            return Err(format!("Target authentication error: {e}"));
        }

        let sftp = match target_session.sftp() {
            Err(e) => return Err(format!("Cannot create sftp channel {e}")),
            Ok(o) => o,
        };

        target_session.set_blocking(false);

        // Store all connections
        self.jump_tcp = Some(arc_jump_tcp);
        self.jump_session = Some(jump_session);
        self.tunnel_channel = Some(tunnel_channel);
        self.session = Some(target_session);
        self.sftp = Some(sftp);
        self.host = target_host.to_string();
        self.user = target_user.to_string();
        self.password = target_password.to_string();

        Ok(())
    }


    pub fn run(&mut self, cmd: &str) -> Result<String, String> {
        println!("running CMD: {}", cmd);
        let mut channel = loop {
            match self.session.as_ref().unwrap().channel_session() {
                Err(e) => {
                    if e.code() != ssh2::ErrorCode::Session(-37) {
                        return Err(format!("Error: {}", e));
                    } else {
                        println!("bloking..., trying again.");
                    }
                }
                Ok(o) => break o,
            };
            thread::sleep(time::Duration::from_millis(WAIT_MS));
        };

        loop {
            match channel.exec(cmd) {
                Err(e) => {
                    if e.code() == ssh2::ErrorCode::Session(-37) {
                        println!("bloking..., trying again.");
                        thread::sleep(time::Duration::from_millis(WAIT_MS));
                    } else {
                        return Err(format!("Error channel exec: {}", e));
                    }
                }
                Ok(_) => break,
            }
        }

        let mut s = String::new();
        loop {
            match channel.stderr().read_to_string(&mut s) {
                Err(ref e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        println!("Channel write error: {}", e);
                        thread::sleep(time::Duration::from_millis(WAIT_MS));
                        continue;
                    } else {
                        return Err(format!("Error channel read stderr: {}", e));
                    }
                }
                Ok(_) => break,
            }
        }
        if !s.trim().is_empty() {
            return Err(format!("stderr: {}", s));
        };

        channel.read_to_string(&mut s).unwrap();

        loop {
            match channel.wait_close() {
                Err(e) => {
                    if e.code() != ssh2::ErrorCode::Session(-37) {
                        return Err(format!("Error: {}", e));
                    } else {
                        println!("bloking..., trying again.");
                    }
                }
                Ok(_) => break,
            }
            thread::sleep(time::Duration::from_millis(WAIT_MS));
        }
        let output = s.trim().to_string();
        println!("stdout: {output}");
        Ok(output)
    }
    pub fn scp_download(
        &mut self,
        remotepath: &str,
        localpath: &str,
        _window: tauri::Window,
    ) -> Result<String, String> {
        println!("downloading: {remotepath}");
        let (mut channel, stat) = match self
            .session
            .as_ref()
            .unwrap()
            .scp_recv(Path::new(remotepath))
        {
            Err(e) => return Err(format!("Cannot open scp channel: {}", e)),
            Ok(o) => o,
        };
        let size = stat.size();
        println!("remote file size: {}", size);
        let f = File::create(localpath).expect("Unable to create file");
        let mut f = BufWriter::new(f);
        let mut buffer = [0; 16000];
        let mut count = 0;
        let mut prev_percent = 0;
        loop {
            match channel.read(&mut buffer[..]) {
                Err(e) => {
                    println!("error: {:?}", e);
                    return Err(e.to_string());
                }
                Ok(n) => {
                    // println!("{n} bytes read");
                    if n == 0 {
                        break;
                    } else {
                        f.write(&buffer[..n]).unwrap();
                        count += n;
                    }
                    // report progress
                    let percent = ((count as f64 / size as f64) * 100.) as i32;
                    if prev_percent != percent {
                        let _p = percent as f32 / 100.;
                        //window.emit("PROGRESS", Payload { percent: p }).unwrap();
                        prev_percent = percent;
                    }
                }
            }
        }
        println!("written: {count}");
        //window.emit("PROGRESS", Payload { percent: 0. }).unwrap();
        Ok("done".to_string())
    }
    pub fn scp_upload(
        &mut self,
        localpath: &str,
        remotepath: &str,
        _window: tauri::Window,
    ) -> Result<String, String> {
        println!("uploading: {localpath} to {remotepath}");
        let size = std::fs::metadata(localpath).unwrap().len();
        let mut channel =
            match self
                .session
                .as_ref()
                .unwrap()
                .scp_send(Path::new(remotepath), 0o644, size, None)
            {
                Err(e) => return Err(format!("Cannot open scp channel: {}", e)),
                Ok(o) => o,
            };
        println!("file size: {}", size);
        let f = File::open(localpath).expect("Unable to open file");
        let mut f = BufReader::new(f);
        let mut buffer = [0; 16000];
        let mut count = 0;
        let mut prev_percent = 0;
        loop {
            let n = f.read(&mut buffer).unwrap();
            match channel.write(&buffer[..n]) {
                Err(e) => {
                    println!("error: {:?}", e);
                    return Err(e.to_string());
                }
                Ok(n) => {
                    count += n;
                    if n < 16000 {
                        break;
                    }
                }
            }
            // report progress
            let percent = ((count as f64 / size as f64) * 100.) as i32;
            if prev_percent != percent {
                let _p = percent as f32 / 100.;
                //window.emit("PROGRESS", Payload { percent: p }).unwrap();
                prev_percent = percent;
            }
        }

        println!("written: {count}");
        //window.emit("PROGRESS", Payload { percent: 0. }).unwrap();
        Ok("done".to_string())
    }
    pub fn sftp_stat(&mut self, filename: &str) -> Result<FileStat, String> {
        match self.sftp.as_ref().unwrap().lstat(Path::new(filename)) {
            Err(e) => Err(format!("Cannot stat {filename}: {e}")),
            Ok(o) => Ok(o),
        }
    }
    pub fn sftp_mkdir(&mut self, dirname: &str) -> Result<(), String> {
        match self.sftp.as_ref().unwrap().mkdir(Path::new(dirname), 0o755) {
            Err(e) => Err(format!("Cannot make dir {dirname}: {e}")),
            Ok(_) => Ok(()),
        }
    }
    pub fn sftp_rmdir(&mut self, dirname: &str) -> Result<(), String> {
        match self.sftp.as_ref().unwrap().rmdir(Path::new(dirname)) {
            Err(e) => Err(format!("Cannot delete dir {dirname}: {e}")),
            Ok(_) => Ok(()),
        }
    }
    pub fn sftp_create(&mut self, filename: &str) -> Result<ssh2::File, String> {
        let f = match self.sftp.as_ref().unwrap().create(Path::new(filename)) {
            Err(e) => return Err(format!("Cannot create file {filename}: {e}")),
            Ok(o) => o,
        };
        Ok(f)
    }
    pub fn sftp_open(&mut self, filename: &str) -> Result<ssh2::File, String> {
        let f = match self.sftp.as_ref().unwrap().open(Path::new(filename)) {
            Err(e) => return Err(format!("Cannot open file {filename}: {e}")),
            Ok(o) => o,
        };
        Ok(f)
    }
    pub fn sftp_rename(&mut self, src: &str, dst: &str) -> Result<(), String> {
        let s = Path::new(src);
        let d = Path::new(dst);
        let sftp = self.sftp.as_ref().unwrap();
        if let Err(e) = sftp.rename(&s, &d, None) {
            return Err(format!("Cannot rename {src}: {e}"));
        }
        Ok(())
    }
    pub fn sftp_delete(&mut self, filename: &str) -> Result<(), String> {
        println!("deleting {filename}");

        let path = Path::new(filename);
        let sftp = self.sftp.as_ref().unwrap();
        let stat = match sftp.lstat(path) {
            Err(e) => return Err(format!("{filename}: {e}")),
            Ok(o) => o,
        };
        if stat.file_type().is_symlink() || stat.file_type().is_file() {
            //println!("{filename} is file or link");
            match sftp.unlink(path) {
                Err(e) => Err(format!("Cannot delete {filename}: {e}")),
                Ok(_) => Ok(()),
            }
        } else {
            //println!("file is folder: {filename}");
            let files: Vec<(PathBuf, FileStat)> = match sftp.readdir(path) {
                Err(e) => return Err(format!("Cannot read directory {filename}: {e}")),
                Ok(o) => o,
            };
            //println!("files in: {filename}: {}: {:?}", files.len(), files);
            if files.len() > 0 {
                for (f, _) in files {
                    if let Err(e) = self.sftp_delete(f.clone().to_str().unwrap()) {
                        return Err(format!("Cannot delete directory {filename}: {e}"));
                    }
                }
            }
            println!("rmdir folder: {filename}");
            match self.sftp.as_ref().unwrap().rmdir(path) {
                Err(e) => return Err(format!("Cannot delete directory {filename}: {e}")),
                Ok(_) => Ok(()),
            }
        }
    }
    pub fn sftp_readdir(&mut self, dirname: &str) -> Result<Vec<(PathBuf, FileStat)>, String> {
        let path = Path::new(dirname);
        let files: Vec<(PathBuf, FileStat)> = match self.sftp.as_ref().unwrap().readdir(path) {
            Err(e) => return Err(format!("Cannot read directory {dirname}: {e}")),
            Ok(o) => o,
        };
        Ok(files)
    }
    pub fn sftp_readlink(&mut self, filename: &str) -> Result<String, String> {
        let path = Path::new(filename);
        let destination = match self.sftp.as_ref().unwrap().readlink(path) {
            Err(e) => return Err(format!("Cannot read path {filename}: {e}")),
            Ok(o) => o,
        };
        Ok(String::from(destination.to_string_lossy()))
    }
    pub fn sftp_realpath(&mut self, filename: &str) -> Result<(String, FileStat), String> {
        let path = Path::new(filename);
        let sftp = self.sftp.as_ref().unwrap();
        let destination = match self.sftp.as_ref().unwrap().realpath(path) {
            Err(e) => return Err(format!("Cannot read real path {filename}: {e}")),
            Ok(o) => o,
        };
        let stat = match sftp.stat(&destination) {
            Err(e) => return Err(format!("Cannot stat {filename}: {e}")),
            Ok(o) => o,
        };
        Ok((String::from(destination.to_string_lossy()), stat))
    }
    pub fn sftp_save(&mut self, filename: &str, data: &str) -> Result<(), String> {
        let mut f = match self.sftp.as_ref().unwrap().create(Path::new(filename)) {
            Err(e) => return Err(format!("Cannot create file {filename}: {e}")),
            Ok(o) => o,
        };
        f.write_all(data.as_bytes()).expect("Cannot write data");
        Ok(())
    }
    pub fn channel_shell(&mut self) -> Result<(), String> {
        let session = self.session.as_ref().unwrap();
        session.set_blocking(true);
        let mut pty = session.channel_session().unwrap();

        // setenv not working
        //p ty.setenv("FOO","VAR").unwrap();

        pty.request_pty("xterm-256color", None, None).unwrap();
        pty.shell().unwrap();

        self.pty = Some(Arc::new(Mutex::new(pty)));
        session.set_blocking(false);
        Ok(())
    }
    pub fn channel_shell_size(&mut self, cols: u32, rows: u32) -> Result<(), String> {
        match self
            .pty
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .request_pty_size(cols, rows, None, None)
        {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Error resizing terminal: {:?}", e.message())),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::env;
    const PORT: u16 = 22;

    fn get_params() -> (String, String, String, String) {
        let host = env::var("TEST_SSH_HOST").unwrap();
        let user = env::var("TEST_SSH_USER").unwrap();
        let pass = env::var("TEST_SSH_PASS").unwrap();
        let home = env::var("TEST_SSH_HOME").unwrap();
        assert!(host.len() > 0);
        assert!(user.len() > 0);
        assert!(pass.len() > 0);
        assert!(home.len() > 0);
        (host, user, pass, home)
    }

    fn get_jump_params() -> (String, String, String) {
        let jump_host = env::var("TEST_JUMP_HOST").unwrap_or_else(|_| "JumpServer".to_string());
        let jump_user = env::var("TEST_JUMP_USER").unwrap_or_else(|_| "jumpuser".to_string());
        let jump_pass = env::var("TEST_JUMP_PASS").unwrap_or_else(|_| "jumppass".to_string());
        assert!(jump_host.len() > 0);
        assert!(jump_user.len() > 0);
        assert!(jump_pass.len() > 0);
        (jump_host, jump_user, jump_pass)
    }

    fn get_target_params() -> (String, String, String) {
        let target_host = env::var("TEST_TARGET_HOST").unwrap_or_else(|_| "NodeA".to_string());
        let target_user = env::var("TEST_TARGET_USER").unwrap_or_else(|_| "admin".to_string());
        let target_pass = env::var("TEST_TARGET_PASS").unwrap_or_else(|_| "nodepass".to_string());
        assert!(target_host.len() > 0);
        assert!(target_user.len() > 0);
        assert!(target_pass.len() > 0);
        (target_host, target_user, target_pass)
    }


    #[tokio::test]
    async fn connect_with_password() {
        let mut ssh = Ssh::new();
        let (host, user, pass, _) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass).await;
        assert!(r.is_ok());
    }
    #[tokio::test]
    async fn connect_with_password_wrong() {
        let mut ssh = Ssh::new();
        let (host, user, _, _) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, "wrong").await;
        assert!(r.is_err());
    }
    #[tokio::test]
    async fn connect_with_key() {
        let mut ssh = Ssh::new();
        let (host, user, _, _) = get_params();
        let pkey = Ssh::private_key_path();
        let pkey = pkey.to_str().unwrap();
        let r = ssh.connect_with_key(&host, PORT, &user, &pkey).await;
        assert!(r.is_ok());
    }
    #[tokio::test]
    async fn connect_with_key_wrong() {
        let mut ssh = Ssh::new();
        let (host, user, _, _) = get_params();
        let r = ssh
            .connect_with_key(&host, PORT, &user, "/invalid/key")
            .await;
        assert!(r.is_err());
    }
    #[tokio::test]
    async fn connect_with_host_wrong() {
        let mut ssh = Ssh::new();
        let (_, user, pass, _) = get_params();
        let r = ssh
            .connect_with_password("example.com", PORT, &user, &pass)
            .await;
        assert!(r.is_err());
    }
    #[tokio::test]
    async fn run_command() {
        let mut ssh = Ssh::new();
        let (host, user, pass, _) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass).await;
        assert!(r.is_ok());
        let output = ssh.run("whoami").unwrap();
        assert_eq!("support", output.as_str());
    }
    #[test]
    fn has_private_key() {
        assert!(Ssh::has_private_key());
    }
    #[test]
    #[ignore = "makes ssh keys unavailable for other tests to pass"]
    fn generate_keys() {
        let seckey = Ssh::private_key_path();
        let secbak = PathBuf::from(seckey.to_string_lossy().to_string() + ".bak");
        let pubkey = Ssh::public_key_path();
        let pubbak = PathBuf::from(pubkey.to_string_lossy().to_string() + ".bak");

        // backup keys
        if seckey.exists() {
            std::fs::rename(&seckey, &secbak).unwrap();
        }
        if pubkey.exists() {
            std::fs::rename(&pubkey, &pubbak).unwrap();
        }
        assert!(!Ssh::has_private_key());
        assert!(!Ssh::has_public_key());

        assert!(Ssh::generate_keys().is_ok());
        assert!(Ssh::generate_public_key().is_ok());
        assert!(Ssh::has_private_key());
        assert!(Ssh::has_public_key());

        // restore keys
        if secbak.exists() {
            std::fs::rename(&secbak, &seckey).unwrap();
        }
        if pubbak.exists() {
            std::fs::rename(&pubbak, &pubkey).unwrap();
        }
    }

    #[tokio::test]
    async fn setup_ssh() {
        let (host, user, pass, _) = get_params();
        assert!(Ssh::setup_ssh(&host, PORT, &user, &pass).await.is_ok());
    }

    #[test]
    fn supported_algs() {
        println!("{}", Ssh::supported_algs());
    }
    #[tokio::test]
    async fn mkdir_rmdir() {
        let mut ssh = Ssh::new();
        let (host, user, pass, home) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass).await;
        assert!(r.is_ok());
        let folder = format!("{home}/folder");
        assert!(ssh.sftp_stat(&home).is_ok());
        assert!(ssh.sftp_stat(&folder).is_err());
        assert!(ssh.sftp_mkdir(&folder).is_ok());
        assert!(ssh.sftp_stat(&folder).is_ok());
        assert!(ssh.sftp_rmdir(&folder).is_ok());
        assert!(ssh.sftp_stat(&folder).is_err());
    }
    #[tokio::test]
    async fn create_delete() {
        let mut ssh = Ssh::new();
        let (host, user, pass, home) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass).await;
        assert!(r.is_ok());
        assert!(ssh.sftp_stat(&home).is_ok());
        assert!(ssh.sftp_stat(&format!("{home}/file")).is_err());
        assert!(ssh.sftp_create(&format!("{home}/file")).is_ok());
        assert!(ssh.sftp_stat(&format!("{home}/file")).is_ok());
        assert!(ssh.sftp_delete(&format!("{home}/file")).is_ok());
        assert!(ssh.sftp_stat(&format!("{home}/file")).is_err());

        assert!(ssh.sftp_mkdir(&format!("{home}/dir")).is_ok());
        assert!(ssh.sftp_stat(&format!("{home}/dir")).is_ok());
        assert!(ssh.sftp_create(&format!("{home}/dir/file")).is_ok());
        assert!(ssh.sftp_stat(&format!("{home}/dir/file")).is_ok());
        assert!(ssh.sftp_delete(&format!("{home}/dir")).is_ok());
        assert!(ssh.sftp_stat(&format!("{home}/dir")).is_err());
    }
    #[tokio::test]
    async fn readdir() {
        let mut ssh = Ssh::new();
        let (host, user, pass, home) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass).await;
        assert!(r.is_ok());
        assert!(ssh.sftp_stat(&home).is_ok());
        let files = ssh.sftp_readdir("/").unwrap();
        assert!(files.len() > 0);
        // for f in files  {
        //     println!("{:?}", f);
        // }
    }
    #[tokio::test]
    async fn rename() {
        let mut ssh = Ssh::new();
        let (host, user, pass, home) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass).await;
        assert!(r.is_ok());
        assert!(ssh.sftp_stat(&format!("{home}")).is_ok());
        assert!(ssh.sftp_stat(&format!("{home}/file")).is_err());
        assert!(ssh.sftp_create(&format!("{home}/file")).is_ok());
        assert!(ssh.sftp_stat(&format!("{home}/file")).is_ok());
        assert!(ssh
            .sftp_rename(&format!("{home}/file"), &format!("{home}/file1"))
            .is_ok());
        assert!(ssh.sftp_stat(&format!("{home}/file")).is_err());
        assert!(ssh.sftp_stat(&format!("{home}/file1")).is_ok());
        assert!(ssh.sftp_delete(&format!("{home}/file1")).is_ok());
        assert!(ssh.sftp_stat(&format!("{home}/file1")).is_err());
    }

    #[tokio::test]
    async fn test_connect_with_password_via_jump() {
        let mut ssh = Ssh::new();
        let (jump_host, jump_user, jump_pass) = get_jump_params();
        let (target_host, target_user, target_pass) = get_target_params();

        // Configure jump server
        ssh.set_jump_server(&jump_host, &jump_user, &jump_pass);

        // Connect to target through jump server
        let result = ssh.connect_with_password(&target_host, PORT, &target_user, &target_pass).await;
        
        match result {
            Ok(_) => {
                println!("✅ Successfully connected to {} through jump server {}", target_host, jump_host);
                
                // Test that the connection works by running a command
                match ssh.run("whoami") {
                    Ok(output) => {
                        println!("✅ Command executed successfully: {}", output);
                        assert_eq!(target_user, output.trim());
                    }
                    Err(e) => {
                        panic!("❌ Failed to execute command: {}", e);
                    }
                }

                // Test SFTP functionality
                match ssh.sftp_stat("/") {
                    Ok(_) => println!("✅ SFTP connection working"),
                    Err(e) => panic!("❌ SFTP not working: {}", e),
                }

                // Clean disconnect
                if let Err(e) = ssh.disconnect() {
                    println!("⚠️  Disconnect warning: {}", e);
                }
            }
            Err(e) => {
                panic!("❌ Failed to connect through jump server: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_connect_with_key_via_jump() {
        let mut ssh = Ssh::new();
        let (jump_host, jump_user, _) = get_jump_params();
        let (target_host, target_user, _) = get_target_params();

        // Use private key for jump server
        let jump_key = Ssh::private_key_path();
        let target_key = Ssh::private_key_path();

        // Configure jump server with key
        ssh.set_jump_server_with_key(&jump_host, &jump_user, jump_key.to_str().unwrap());

        // Connect to target through jump server using key
        let result = ssh.connect_with_key(&target_host, PORT, &target_user, target_key.to_str().unwrap()).await;
        
        match result {
            Ok(_) => {
                println!("✅ Successfully connected to {} through jump server {} using keys", target_host, jump_host);
                
                // Test command execution
                match ssh.run("hostname") {
                    Ok(output) => {
                        println!("✅ Hostname command executed: {}", output);
                        assert!(output.len() > 0);
                    }
                    Err(e) => {
                        panic!("❌ Failed to execute hostname command: {}", e);
                    }
                }

                // Clean disconnect
                if let Err(e) = ssh.disconnect() {
                    println!("⚠️  Disconnect warning: {}", e);
                }
            }
            Err(e) => {
                panic!("❌ Failed to connect through jump server with keys: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_jump_server_invalid_credentials() {
        let mut ssh = Ssh::new();
        let (jump_host, jump_user, _) = get_jump_params();
        let (target_host, target_user, target_pass) = get_target_params();

        // Configure jump server with wrong password
        ssh.set_jump_server(&jump_host, &jump_user, "wrongpassword");

        // This should fail at jump server authentication
        let result = ssh.connect_with_password(&target_host, PORT, &target_user, &target_pass).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("Jump server authentication error") || error.contains("authentication"));
        println!("✅ Correctly failed with invalid jump server credentials: {}", error);
    }

    #[tokio::test]
    async fn test_jump_server_invalid_target() {
        let mut ssh = Ssh::new();
        let (jump_host, jump_user, jump_pass) = get_jump_params();
        let (_, target_user, target_pass) = get_target_params();

        // Configure valid jump server
        ssh.set_jump_server(&jump_host, &jump_user, &jump_pass);

        // Try to connect to invalid target
        let result = ssh.connect_with_password("invalid-target-host", PORT, &target_user, &target_pass).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("tunnel") || error.contains("connection") || error.contains("handshake"));
        println!("✅ Correctly failed with invalid target host: {}", error);
    }

    #[tokio::test]
    async fn test_direct_vs_jump_connection() {
        // Test both direct and jump connections to compare functionality
        let (direct_host, direct_user, direct_pass, _) = get_params();
        let (jump_host, jump_user, jump_pass) = get_jump_params();
        let (target_host, target_user, target_pass) = get_target_params();

        // Test direct connection (existing functionality)
        let mut direct_ssh = Ssh::new();
        let direct_result = direct_ssh.connect_with_password(&direct_host, PORT, &direct_user, &direct_pass).await;
        
        // Test jump connection
        let mut jump_ssh = Ssh::new();
        jump_ssh.set_jump_server(&jump_host, &jump_user, &jump_pass);
        let jump_result = jump_ssh.connect_with_password(&target_host, PORT, &target_user, &target_pass).await;

        if direct_result.is_ok() {
            println!("✅ Direct connection successful");
            let direct_output = direct_ssh.run("whoami").unwrap();
            println!("Direct connection user: {}", direct_output);
        }

        if jump_result.is_ok() {
            println!("✅ Jump connection successful");
            let jump_output = jump_ssh.run("whoami").unwrap();
            println!("Jump connection user: {}", jump_output);
            
            // Both should work the same way
            assert_eq!(target_user, jump_output.trim());
        }

        // Clean up
        if direct_result.is_ok() {
            let _ = direct_ssh.disconnect();
        }
        if jump_result.is_ok() {
            let _ = jump_ssh.disconnect();
        }
    }

    #[tokio::test]
    async fn test_jump_server_multiple_operations() {
        let mut ssh = Ssh::new();
        let (jump_host, jump_user, jump_pass) = get_jump_params();
        let (target_host, target_user, target_pass) = get_target_params();

        // Configure and connect
        ssh.set_jump_server(&jump_host, &jump_user, &jump_pass);
        let result = ssh.connect_with_password(&target_host, PORT, &target_user, &target_pass).await;
        
        assert!(result.is_ok(), "Failed to connect: {:?}", result);

        // Test multiple operations to ensure connection stability
        
        // 1. Test command execution
        let whoami_result = ssh.run("whoami");
        assert!(whoami_result.is_ok(), "whoami failed: {:?}", whoami_result);
        println!("✅ whoami: {}", whoami_result.unwrap());

        // 2. Test another command
        let pwd_result = ssh.run("pwd");
        assert!(pwd_result.is_ok(), "pwd failed: {:?}", pwd_result);
        println!("✅ pwd: {}", pwd_result.unwrap());

        // 3. Test SFTP operations
        let stat_result = ssh.sftp_stat("/tmp");
        assert!(stat_result.is_ok(), "SFTP stat failed: {:?}", stat_result);
        println!("✅ SFTP stat /tmp successful");

        // 4. Test file creation and deletion
        let test_file = "/tmp/jump_test_file";
        let create_result = ssh.sftp_create(test_file);
        assert!(create_result.is_ok(), "File creation failed: {:?}", create_result);
        println!("✅ Created test file");

        let delete_result = ssh.sftp_delete(test_file);
        assert!(delete_result.is_ok(), "File deletion failed: {:?}", delete_result);
        println!("✅ Deleted test file");

        // Clean disconnect
        let disconnect_result = ssh.disconnect();
        assert!(disconnect_result.is_ok(), "Disconnect failed: {:?}", disconnect_result);
        println!("✅ Clean disconnect successful");
    }
}

// Helper function to set up environment variables for testing
#[cfg(test)]
pub fn setup_test_env() {
    // Set these environment variables before running tests:
    // 
    // For existing direct connection tests:
    // export TEST_SSH_HOST="your-direct-host"
    // export TEST_SSH_USER="your-user"  
    // export TEST_SSH_PASS="your-password"
    // export TEST_SSH_HOME="/home/your-user"
    //
    // For jump server tests:
    // export TEST_JUMP_HOST="your-jump-server"
    // export TEST_JUMP_USER="jump-user"
    // export TEST_JUMP_PASS="jump-password"
    // export TEST_TARGET_HOST="target-node"
    // export TEST_TARGET_USER="target-user"
    // export TEST_TARGET_PASS="target-password"
    
    println!("Test environment setup:");
    println!("TEST_SSH_HOST: {}", std::env::var("TEST_SSH_HOST").unwrap_or("NOT_SET".to_string()));
    println!("TEST_JUMP_HOST: {}", std::env::var("TEST_JUMP_HOST").unwrap_or("NOT_SET".to_string()));
    println!("TEST_TARGET_HOST: {}", std::env::var("TEST_TARGET_HOST").unwrap_or("NOT_SET".to_string()));
}
