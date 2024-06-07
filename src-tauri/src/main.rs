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
// use serde::{Deserialize, Serialize};
// use chrono::prelude::{DateTime, NaiveDateTime, Utc};

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

#[cfg(target_os = "linux")]
extern crate webkit2gtk;

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
    let (id_tx, id_rx): (Sender<i32>, Receiver<i32>) = flume::unbounded();
    
    
    //app.emit_all("terminal-output", Payload {output: "testing event".into()}).unwrap();                          

    {
        let mut ssh = state.ssh.lock().unwrap();
        ssh.channel_shell().unwrap();
    }

    // wirte
    {
        let ssh = state.ssh.lock().unwrap();
        let channel = ssh.channel.as_ref().unwrap();
        let writer = Arc::clone(&channel);
        let mut i = 0;

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
                    i += 1;              
                    id_tx.send(i).unwrap();                
                    println!("{:?}: cmd id sent: {i}", thread::current().id());
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

        thread::spawn(move || loop {
            //println!("{:?}: waiting to recv read signal...", thread::current().id());
            
            match id_rx.try_recv() {
                Ok(cmd_id) => {
                    println!("{:?}: running cmd id {cmd_id}...", thread::current().id());
                    let mut buf = vec![0; 4096];
                    let mut reader = reader.lock().unwrap();
                    //loop {
                        match reader.read(&mut buf) {                                                      
                            Ok(n) => { 
                                if n == 0 {
                                    println!("read is ZERO");
                                    break;
                                }
                                println!("Stdout: {:?}", &buf[0..n]);
                                let result = match String::from_utf8(buf[0..n].to_vec()) {
                                    Ok(o) => o,
                                    Err(e) => panic!("invalid utf-8 sequence: {}", e)
                                };
                                println!("result ({n}):\n{}", result.clone());  
                                app.emit_all("terminal-output", Payload {output: buf[0..n].to_vec()}).unwrap();                                 
                            },
                            Err(e) => {
                                if e.kind() == std::io::ErrorKind::WouldBlock {
                                    println!("blocking reading, trying again");
                                    break;
                                    // thread::sleep(time::Duration::from_millis(200));
                                } else {
                                    println!("Cannot read channel: {e}");   
                                    assert!(false);     
                                    // return Err(format!("Cannot read channel: {e}"));        
                                }
                            },
                        };
                    //}                    
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

        *state.itx.lock().unwrap() = Some(itx);
        //let mutex = state.itx.lock().unwrap();
        //mutex.as_ref().unwrap().send("\n".to_string()).unwrap();

        // itx.send("hostname\n".to_string()).unwrap();
        // thread::sleep(time::Duration::from_millis(1000));
        // itx.send("ls -l /\n".to_string()).unwrap();
        // thread::sleep(time::Duration::from_millis(1000));
        
        

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
