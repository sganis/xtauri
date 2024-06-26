#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]


mod settings;
mod ssh;
mod command;

use std::time;
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::{Manager, Window};
use tauri::State;
use tokio::sync::Mutex;
use mio::net::TcpStream;
use mio::{Events, Interest, Poll, Token};
//use polling::{Event, Events, Poller};
use settings::Settings;


// use serde::{Deserialize, Serialize};
// use chrono::prelude::{DateTime, NaiveDateTime, Utc};

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

#[cfg(target_os = "linux")]
extern crate webkit2gtk;


const WAIT_MS: u64 = 10;

#[derive(Default)]
struct AppState {
    ssh: Mutex<ssh::Ssh>,
    connected: Mutex<bool>,
    itx: Mutex<Option<tokio::sync::mpsc::Sender<String>>>,
}

// the payload type must implement `Serialize` and `Clone`.
#[derive(Clone, serde::Serialize)]
struct Payload {
    data: Vec::<u8>,
    // data: String,
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
async fn connect_with_password(settings: Settings, state: State<'_,AppState>) -> Result<(), String> {
    let mut _ssh = ssh::Ssh::new();
    match _ssh.connect_with_password(
        settings.server.as_str(), 
        settings.port, 
        settings.user.as_str(), 
        settings.password.clone().unwrap().as_str()).await {
        Err(e) => {
            Err(e)
        },
        Ok(_) => {
            write_settings(settings).expect("Cannot write settings");
            let mut ssh = state.ssh.lock().await;
            *ssh = _ssh;
            *state.connected.lock().await = true;
            println!("Connected");
            let output = ssh.run("whoami").unwrap();
            println!("{}", output);
            Ok(())
        }
    }   
}

#[tauri::command]
async fn connect_with_key(settings: Settings, state: State<'_,AppState>) -> Result<(), String> {
    let mut _ssh = ssh::Ssh::new();
    let mut pkey = String::new();
    
    if settings.private_key.clone().unwrap().is_empty() {
        pkey = String::from(ssh::Ssh::private_key_path().to_string_lossy());
    }

    match _ssh.connect_with_key(
        settings.server.as_str(), 
        settings.port, 
        settings.user.as_str(), 
        pkey.as_str()).await {
        Err(e) => {
            println!("{e}");
            Err(e)
        },        
        Ok(_) => {
            write_settings(settings).expect("Cannot write settings");
            let mut ssh = state.ssh.lock().await;
            *ssh = _ssh;
            *state.connected.lock().await = true;
            println!("Connected");
            let output = ssh.run("whoami").unwrap();
            println!("{}", output);
            Ok(())
        }
    }   
}

#[tauri::command]
async fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let mut ssh = state.ssh.lock().await;
    ssh.disconnect()   
}

#[tauri::command]
async fn setup_ssh(settings: Settings) -> Result<(), String> {
    let host = settings.server.as_str();
    let port = settings.port; 
    let user = settings.user.as_str();
    let password = settings.password.unwrap();
    ssh::Ssh::setup_ssh(host, port, user, &password).await
}

#[tauri::command]
async fn ssh_run(command: String, state: State<'_,AppState>) -> Result<String, String> {
    let mut ssh = state.ssh.lock().await;
    ssh.run(&command)
}

#[tauri::command]
async fn download(
    remotepath: String, 
    localpath: String,
    window: Window, 
    state: State<'_, AppState>) -> Result<String, String> {
    let mut ssh = state.ssh.lock().await;
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
    let mut ssh = state.ssh.lock().await;
    match ssh.scp_upload(&localpath, &remotepath, window) {
        Err(e) => Err(e),
        Ok(o) => {
            println!("file uploaded to: {remotepath}");
            Ok(serde_json::to_string(&o).unwrap())
        },
    }
}

#[tauri::command]
fn zoom_window(window: tauri::Window, zoom: f64) {
    //println!("zoom window: {zoom}");

    let _ = window.with_webview(move |webview| {
        #[cfg(target_os = "linux")]
        {
          // see https://docs.rs/webkit2gtk/0.18.2/webkit2gtk/struct.WebView.html
          // and https://docs.rs/webkit2gtk/0.18.2/webkit2gtk/trait.WebViewExt.html
          use webkit2gtk::traits::WebViewExt;
          webview.inner().set_zoom_level(zoom);
        }

        #[cfg(windows)]
        unsafe {
          // see https://docs.rs/webview2-com/0.19.1/webview2_com/Microsoft/Web/WebView2/Win32/struct.ICoreWebView2Controller.html
          webview.controller().SetZoomFactor(zoom).unwrap();
        }

        #[cfg(target_os = "macos")]
        unsafe {
          let () = msg_send![webview.inner(), setPageZoom: zoom];
        }
      });
}

#[tauri::command]
async fn send_key(key: String, state: State<'_,AppState>) -> Result<(), String> {
    println!("key: {key}");
    let mutex = state.itx.lock().await;
    mutex.as_ref().unwrap().send(key).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_terminal(state: State<'_,AppState>, app: tauri::AppHandle) -> Result<(), String> {
    
    let (itx, mut irx) = tokio::sync::mpsc::channel(100);
    //let (itx, irx) = flume::unbounded();
    *state.itx.lock().await = Some(itx);

    // create tty shell
    {
        let mut ssh = state.ssh.lock().await;
        ssh.channel_shell().unwrap();
    }

    // write
    {
        let ssh = state.ssh.lock().await;    
        let channel = ssh.channel.as_ref().unwrap();
        let writer = Arc::clone(&channel);

        tokio::spawn(async move { 
            loop {
                //println!("{:?}: waiting to recv command...", thread::current().id());
                if let Some(cmd) = irx.recv().await {
                    println!("command: {cmd}");
                    let mut writer = writer.lock().await;                    
                    match writer.write(cmd.as_bytes()) {
                        Ok(0) => {
                            // Connection closed
                            panic!("Connection closed by server.");                                
                        }
                        Ok(_n) => {
                            // Process the data
                            //println!("stdin: \n{}\nend stdin", cmd);
                            continue;
                        }
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                println!("Channel write error: {}", e);
                                tokio::time::sleep(time::Duration::from_millis(WAIT_MS)).await;         
                                //continue;
                            } else {
                                panic!("Error reading from channel: {:?}", e);                                
                            }
                        }
                    }
                }
            }        
        });      
        
    }

    // read 
    {
        let reader;
        let std_tcp;
        {
            let lock_ssh = state.ssh.lock().await;  
            let channel = lock_ssh.channel.as_ref().unwrap();  
            reader = Arc::clone(channel);
            let tcp = lock_ssh.tcp.as_ref().unwrap();
            let lock_tcp = tcp.lock().await;
            std_tcp = lock_tcp.try_clone().unwrap();
        }
        let mut buf = vec![0; 4096];
        

        tokio::spawn(async move {
            let mut poller = Poll::new().unwrap();
            let mut mio_tcp = TcpStream::from_std(std_tcp);
            poller.registry().register(&mut mio_tcp, Token(0), Interest::READABLE).unwrap();
            let mut events = Events::with_capacity(128);
            
            // let poller = Poller::new().unwrap();
            // unsafe {poller.add(&std_tcp, Event::readable(1)).unwrap()};
            // let mut events = Events::new();    

            loop {
                println!("Polling...");
                //events.clear();
                //poller.wait(&mut events, None).unwrap();
                poller.poll(&mut events, None).unwrap();
                //println!("Polling: data recieved");              
                let mut reader = reader.lock().await;   
                        
                if let Some(ev) = events.iter().next() {
                    println!("EVENT: {:?}", ev);
                    if ev.token() == Token(0) {

                                                 
                        match reader.read(&mut buf) {                                                      
                            Ok(n) => { 
                                if n == 0 {
                                    panic!("read is ZERO");
                                }
                                //println!("Stdout: {:?}", &buf[0..n]);
                                // let data = match String::from_utf8(buf[..n].to_vec()) {
                                //     Ok(o) => o,
                                //     Err(e) => panic!("invalid utf-8 sequence: {}", e)
                                // };
                                //println!("result ({n}):\n{}", result.clone());  
                                app.emit_all("terminal-output", Payload {data: buf[..n].to_vec()}).unwrap();                                 
                                //app.emit_all("terminal-output", Payload {data}).unwrap();                                 
                                
                            },
                            Err(e) => {
                                if e.kind() == std::io::ErrorKind::WouldBlock {
                                    println!("blocking reading, trying again");
                                    //tokio::time::sleep(time::Duration::from_millis(WAIT_MS)).await;
                                    // TODO: 
                                    // poll_for_new_data();
                                } else {
                                    panic!("Cannot read channel: {e}");   
                                }
                            },
                        }
                        //poller.modify(&std_tcp, Event::readable(1)).unwrap();  
                        // registry.reregister(connection, event.token(), Interest::READABLE)?                  
                        poller.registry().reregister(&mut mio_tcp, Token(0), Interest::READABLE).unwrap();
            
                    }                    
                };                                           
            }            
        });
    }

    Ok(())

}

#[tokio::main]
async fn main() {
    // use tracing_subscriber::{filter, fmt, prelude::*};
    // tracing_subscriber::registry()
    //     .with(filter::EnvFilter::from_default_env())
    //     .with(fmt::Layer::default())
    //     .init();
    // tracing::debug!("Starting...");


    tauri::async_runtime::set(tokio::runtime::Handle::current());


    tauri::Builder::default()     
        .setup(|app| {
            app.manage(AppState::default());
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
