#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]


mod settings;
mod ssh;
mod command;

use flume::{Sender, Receiver};
use tauri::{Manager, Window};
use settings::Settings;
use std::sync::{Arc, Mutex};
use tauri::State;
use std::io::{Read, BufReader, Write, BufWriter};
use std::{thread, time};
use mio::net::TcpStream as MioTcpStream;
use mio::{Events, Interest, Poll, Token};

// use serde::{Deserialize, Serialize};
// use chrono::prelude::{DateTime, NaiveDateTime, Utc};

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

#[cfg(target_os = "linux")]
extern crate webkit2gtk;

const CLIENT: Token = Token(0);

#[derive(Default)]
struct AppState {
    ssh: Mutex<ssh::Ssh>,
    connected: Mutex<bool>,
    itx: Mutex<Option<Sender<String>>>,
}

// the payload type must implement `Serialize` and `Clone`.
#[derive(Clone, serde::Serialize)]
struct Payload {
  output: Vec::<u8>,
}

#[tauri::command]
fn read_settings() -> Result<Settings, String> {
    settings::read_settings()
}

#[tauri::command]
fn write_settings(settings: Settings) -> Result<(), String> {
    settings::write_settings(settings)
}
#[tauri::command]
fn connect_with_password(settings: Settings, state: State<'_,AppState>) -> Result<(), String> {
    let mut _ssh = ssh::Ssh::new();
    match _ssh.connect_with_password(
        settings.server.as_str(), 
        settings.port, 
        settings.user.as_str(), 
        settings.password.clone().unwrap().as_str()) {
        Err(e) => {
            Err(e)
        },
        Ok(_) => {
            write_settings(settings).expect("Cannot write settings");
            let mut ssh = state.ssh.lock().unwrap();
            *ssh = _ssh;
            *state.connected.lock().unwrap() = true;
            println!("Connected");
            let output = ssh.run("whoami").unwrap();
            println!("{}", output);
            Ok(())
        }
    }   
}

#[tauri::command]
fn connect_with_key(settings: Settings, state: State<'_,AppState>) -> Result<(), String> {
    let mut _ssh = ssh::Ssh::new();
    let mut pkey = String::new();
    
    if settings.private_key.clone().unwrap().is_empty() {
        pkey = String::from(ssh::Ssh::private_key_path().to_string_lossy());
    }

    match _ssh.connect_with_key(
        settings.server.as_str(), 
        settings.port, 
        settings.user.as_str(), 
        pkey.as_str()) {
        Err(e) => {
            println!("{e}");
            Err(e)
        },        
        Ok(_) => {
            write_settings(settings).expect("Cannot write settings");
            let mut ssh = state.ssh.lock().unwrap();
            *ssh = _ssh;
            *state.connected.lock().unwrap() = true;
            println!("Connected");
            let output = ssh.run("whoami").unwrap();
            println!("{}", output);
            Ok(())
        }
    }   
}

#[tauri::command]
fn disconnect(app: State<AppState>) -> Result<(), String> {
    let mut ssh = app.ssh.lock().unwrap();
    ssh.disconnect()   
}

#[tauri::command]
async fn setup_ssh(settings: Settings) -> Result<(), String> {
    let host = settings.server.as_str();
    let port = settings.port; 
    let user = settings.user.as_str();
    let password = settings.password.unwrap();
    ssh::Ssh::setup_ssh(host, port, user, &password)
}

#[tauri::command]
async fn ssh_run(command: String, state: State<'_,AppState>) -> Result<String, String> {
    let mut ssh = state.ssh.lock().unwrap();
    ssh.run(&command)
}

#[tauri::command]
async fn download(
    remotepath: String, 
    localpath: String,
    window: Window, 
    state: State<'_, AppState>) -> Result<String, String> {
    let mut ssh = state.ssh.lock().unwrap();
    match ssh.scp_download(&remotepath, &localpath, window) {
        Err(e) => Err(e),
        Ok(o) => {
            println!("file saved to: {localpath}");
            Ok(serde_json::to_string(&o).unwrap())
        },
    }
}

#[tauri::command]
async fn upload(
    localpath: String,
    remotepath: String, 
    window: Window,
    state: State<'_,AppState>) -> Result<String, String> {
    let mut ssh = state.ssh.lock().unwrap();
    match ssh.scp_upload(&localpath, &remotepath, window) {
        Err(e) => Err(e),
        Ok(o) => {
            println!("file uploaded to: {remotepath}");
            Ok(serde_json::to_string(&o).unwrap())
        },
    }
}

#[tauri::command]
fn zoom_window(window: tauri::Window, scale_factor: f64) {
    let _ = window.with_webview(move |webview| {
        #[cfg(target_os = "linux")]
        {
          // see https://docs.rs/webkit2gtk/0.18.2/webkit2gtk/struct.WebView.html
          // and https://docs.rs/webkit2gtk/0.18.2/webkit2gtk/trait.WebViewExt.html
          use webkit2gtk::traits::WebViewExt;
          webview.inner().set_zoom_level(scale_factor);
        }

        #[cfg(windows)]
        unsafe {
          // see https://docs.rs/webview2-com/0.19.1/webview2_com/Microsoft/Web/WebView2/Win32/struct.ICoreWebView2Controller.html
          webview.controller().SetZoomFactor(scale_factor).unwrap();
        }

        #[cfg(target_os = "macos")]
        unsafe {
          let () = msg_send![webview.inner(), setPageZoom: scale_factor];
        }
      });
}

#[tauri::command]
async fn send_key(key: String, state: State<'_,AppState>) -> Result<(), String> {
    println!("key: {key}");
    let mutex = state.itx.lock().unwrap();
    mutex.as_ref().unwrap().send(key).unwrap();
    Ok(())
}


#[tauri::command]
async fn open_terminal(state: State<'_,AppState>, app: tauri::AppHandle) -> Result<(), String> {
    let (itx, irx): (Sender<String>, Receiver<String>) = flume::unbounded();
    *state.itx.lock().unwrap() = Some(itx);

    // create tty shell
    {
        let mut ssh = state.ssh.lock().unwrap();
        ssh.channel_shell().unwrap();
    }

    // write
    {
        let ssh = state.ssh.lock().unwrap();    
        let channel = ssh.channel.as_ref().unwrap();
        let writer = Arc::clone(&channel);

        thread::spawn(move || loop {
            //println!("{:?}: waiting to recv command...", thread::current().id());
            match irx.try_recv() {
                Ok(cmd) => {
                    println!("{:?}: command: {cmd}", thread::current().id());
                    let mut writer = writer.lock().unwrap();
                    writer.write(cmd.as_bytes()).unwrap();
                    if cmd == "\n" || cmd == "\r" {
                        writer.flush().unwrap();  
                    }
                },
                Err(flume::TryRecvError::Empty) => {
                    thread::sleep(time::Duration::from_millis(10));                     
                }
                Err(e) => {
                    println!("{:?}: Error in id_rx.try_recv(): {e}", thread::current().id());
                    break;
                }
            }
        });
    }

    // read 
    {
        let ssh = state.ssh.lock().unwrap();  
        let channel = ssh.channel.as_ref().unwrap();  
        let reader = Arc::clone(&channel);
        let tcp = ssh.tcp.as_ref().unwrap();
        let mut mio_tcp = MioTcpStream::from_std(tcp.try_clone().unwrap());
        let mut events = Events::with_capacity(128);
        let mut buf = [0; 1024];
        let mut poll = Poll::new().unwrap();
        poll.registry().register(&mut mio_tcp, CLIENT, Interest::READABLE | Interest::WRITABLE).unwrap();
        

        thread::spawn(move || loop {
            println!("Polling...");
            // poll.poll(&mut events, Some(time::Duration::from_millis(200))).unwrap();
            poll.poll(&mut events, None).unwrap();

            for event in events.iter() {
                println!("EVENT: {:?}", event);
                match event.token() {
                    CLIENT => {
                        if event.is_readable() {
                            let mut reader = reader.lock().unwrap();
                            match reader.read(&mut buf) {
                                Ok(0) => {
                                    // Connection closed
                                    panic!("Connection closed by server.");                                
                                }
                                Ok(n) => {
                                    // Process the data
                                    println!("stdout: \n{}\nend stdout", String::from_utf8_lossy(&buf[..n]));
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    println!("Channel read error: {}", e);
                                    continue;
                                }
                                Err(e) => {
                                    panic!("Error reading from channel: {:?}", e);                                
                                }
                            }                        
                        }    
                        if event.is_writable() {
                            println!("Client writable event");
                        }                
                    }
                    _ => unreachable!(),
                }
            }       
        });
    }

    Ok(())

}

fn main() {

    tauri::Builder::default()     
        .setup(|app| {
            app.manage(AppState::default());

            //app.emit_all("send-data", "output from rust".to_string()).unwrap();
            //let handle = app.handle();
            //let state: tauri::State<AppState> = handle.state();
            //*state.itx.as_ref().unwrap().lock().unwrap() = itx;
            //*state.irx.as_ref().unwrap().lock().unwrap() = irx;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            read_settings, write_settings,
            connect_with_key, connect_with_password, 
            ssh_run, download, upload, setup_ssh, disconnect,
            open_terminal, send_key,
            zoom_window,    
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
