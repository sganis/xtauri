#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod command;
mod settings;
mod ssh;

use std::io::{Read, Write};
use std::time;
use tauri::{Emitter, Manager, State, Window};
// use tokio::sync::Mutex;
use mio::net::TcpStream;
use mio::{Events, Interest, Poll, Token};
use std::sync::{Arc, Mutex};
//use polling::{Event, Events, Poller};
use settings::Settings;

const WAIT_MS: u64 = 50;

// use serde::{Deserialize, Serialize};
// use chrono::prelude::{DateTime, NaiveDateTime, Utc};

#[cfg(target_os = "macos")]
extern crate objc;

#[cfg(target_os = "linux")]
extern crate webkit2gtk;

#[derive(Default)]
struct AppState {
    ssh: Mutex<ssh::Ssh>,
    connected: Mutex<bool>,
    itx: Mutex<Option<std::sync::mpsc::Sender<String>>>,
}

// the payload type must implement `Serialize` and `Clone`.
#[derive(Clone, serde::Serialize)]
struct Payload {
    data: Vec<u8>,
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
async fn connect_with_password(
    settings: Settings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut _ssh = ssh::Ssh::new();
    match _ssh
        .connect_with_password(
            settings.server.as_str(),
            settings.port,
            settings.user.as_str(),
            settings.password.clone().unwrap().as_str(),
        )
        .await
    {
        Err(e) => Err(e),
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
async fn connect_with_key(settings: Settings, state: State<'_, AppState>) -> Result<(), String> {
    let mut _ssh = ssh::Ssh::new();
    let mut pkey = String::new();

    if settings.private_key.clone().unwrap().is_empty() {
        pkey = String::from(ssh::Ssh::private_key_path().to_string_lossy());
    }

    match _ssh
        .connect_with_key(
            settings.server.as_str(),
            settings.port,
            settings.user.as_str(),
            pkey.as_str(),
        )
        .await
    {
        Err(e) => {
            println!("{e}");
            Err(e)
        }
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
async fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let mut ssh = state.ssh.lock().unwrap();
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
async fn ssh_run(command: String, state: State<'_, AppState>) -> Result<String, String> {
    let mut ssh = state.ssh.lock().unwrap();
    ssh.run(&command)
}

#[tauri::command]
async fn download(
    remotepath: String,
    localpath: String,
    window: Window,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut ssh = state.ssh.lock().unwrap();
    match ssh.scp_download(&remotepath, &localpath, window) {
        Err(e) => Err(e),
        Ok(o) => {
            println!("file saved to: {localpath}");
            Ok(serde_json::to_string(&o).unwrap())
        }
    }
}

#[tauri::command]
async fn upload(
    localpath: String,
    remotepath: String,
    window: Window,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut ssh = state.ssh.lock().unwrap();
    match ssh.scp_upload(&localpath, &remotepath, window) {
        Err(e) => Err(e),
        Ok(o) => {
            println!("file uploaded to: {remotepath}");
            Ok(serde_json::to_string(&o).unwrap())
        }
    }
}

#[tauri::command]
async fn send_key(key: String, state: State<'_, AppState>) -> Result<(), String> {
    //println!("key: {key}");
    let mutex = state.itx.lock().unwrap();
    mutex.as_ref().unwrap().send(key).map_err(|e| e.to_string())
}

#[tauri::command]
async fn resize(cols: u32, rows: u32, state: State<'_, AppState>) -> Result<(), String> {
    //println!("resize: {cols}x{rows}");
    let mut ssh = state.ssh.lock().unwrap();
    ssh.channel_shell_size(cols, rows)
}

#[tauri::command]
async fn open_terminal(state: State<'_, AppState>, app: tauri::AppHandle) -> Result<(), String> {
    let (itx, irx) = std::sync::mpsc::channel();
    //let (itx, irx) = flume::unbounded();
    *state.itx.lock().unwrap() = Some(itx);
    let arcapp = Arc::new(app);
    let arcappclone = Arc::clone(&arcapp);
    let mut buf = vec![0; 4096];

    // create tty shell
    {
        let mut ssh = state.ssh.lock().unwrap();
        ssh.channel_shell().unwrap();
    }

    // write
    {
        let ssh = state.ssh.lock().unwrap();
        let pty = ssh.pty.as_ref().unwrap();

        // env vars
        // let mut p = pty.lock().unwrap();
        // p.write(b"export FOO=VAR\n").unwrap();
        // loop {
        //     match p.read(&mut buf) {
        //         Err(e) => {
        //             if e.kind() == std::io::ErrorKind::WouldBlock {
        //                 println!("read WouldBlock: {}", e);
        //                 std::thread::sleep(time::Duration::from_millis(WAIT_MS));
        //             } else {
        //                 panic!("Error reading from channel: {:?}", e);
        //             }
        //         },
        //         Ok(n) => {
        //             if n == 0 {
        //                 panic!("cannot set env var, exiting.");
        //             }
        //             break
        //         }
        //     }
        // }

        let writer = Arc::clone(&pty);

        std::thread::spawn(move || {
            loop {
                //println!("{:?}: waiting to recv command...", thread::current().id());
                if let Ok(cmd) = irx.recv() {
                    //println!("command: {cmd}");
                    let mut writer = writer.lock().unwrap();

                    match writer.write(cmd.as_bytes()) {
                        Ok(0) => {
                            // Connection closed
                            println!("Connection closed by server.");
                            arcapp.exit(1);
                        }
                        Ok(_n) => {
                            // Process the data
                            //println!("stdin: \n{}\nend stdin", cmd);
                            continue;
                        }
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                println!("Write WouldBlock: {}", e);
                                std::thread::sleep(time::Duration::from_millis(WAIT_MS));
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
            let lock_ssh = state.ssh.lock().unwrap();
            let pty = lock_ssh.pty.as_ref().unwrap();
            reader = Arc::clone(pty);
            let tcp = lock_ssh.tcp.as_ref().unwrap();
            let lock_tcp = tcp.lock().unwrap();
            std_tcp = lock_tcp.try_clone().unwrap();
        }

        std::thread::spawn(move || {
            let mut poller = Poll::new().unwrap();
            let mut mio_tcp = TcpStream::from_std(std_tcp);
            poller
                .registry()
                .register(&mut mio_tcp, Token(0), Interest::READABLE)
                .unwrap();
            let mut events = Events::with_capacity(1000);

            'loop1: loop {
                //println!("Polling...");
                poller.poll(&mut events, None).unwrap();
                //println!("Polling: data recieved");

                let mut reader = reader.lock().unwrap();

                if let Some(ev) = events.iter().next() {
                    //println!("EVENT: {:?}", ev);
                    if ev.is_read_closed() {
                        println!("read closed, connection terminated.");
                        arcappclone.exit(1);
                        break 'loop1;
                    }

                    if ev.token() == Token(0) {
                        loop {
                            match reader.read(&mut buf) {
                                Ok(n) => {
                                    if n == 0 {
                                        println!("read is ZERO, exiting...");
                                        reader.close().unwrap();
                                        arcappclone.exit(1);
                                        break 'loop1;
                                    }
                                    //println!("Stdout: {:?}", &buf[0..n]);
                                    // let data = match String::from_utf8(buf[..n].to_vec()) {
                                    //      Ok(o) => o,
                                    //      Err(e) => panic!("invalid utf-8 sequence: {}", e)
                                    // };
                                    // println!("result ({n}):\n{}", data);

                                    arcappclone
                                        .emit(
                                            "terminal-output",
                                            Payload {
                                                data: buf[..n].to_vec(),
                                            },
                                        )
                                        .unwrap();

                                    // let chunk_size = 1000;
                                    // let total_chunks = (buf.len() + chunk_size - 1) / chunk_size;

                                    // for i in 0..total_chunks {
                                    //     let start = i * chunk_size;
                                    //     let end = std::cmp::min(start + chunk_size, buf.len());
                                    //     println!("chunk {} {} {}", i, start, end);
                                    //     app.emit("terminal-output", Payload {data: buf[start..end].to_vec()}).unwrap();

                                    // }
                                }
                                Err(e) => {
                                    if e.kind() == std::io::ErrorKind::WouldBlock {
                                        //println!("blocking reading, trying again");
                                        break;
                                        //tokio::time::sleep(time::Duration::from_millis(WAIT_MS)).await;
                                        // TODO:
                                        // poll_for_new_data();
                                    } else {
                                        panic!("Cannot read channel: {e}");
                                    }
                                }
                            }
                        }
                        // this must be done in windows
                        poller
                            .registry()
                            .reregister(&mut mio_tcp, Token(0), Interest::READABLE)
                            .unwrap();
                    }
                };
            }
        });
    }

    println!("terminal started.");

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
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            app.manage(AppState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            read_settings,
            write_settings,
            connect_with_key,
            connect_with_password,
            ssh_run,
            download,
            upload,
            setup_ssh,
            disconnect,
            open_terminal,
            send_key,
            resize,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
