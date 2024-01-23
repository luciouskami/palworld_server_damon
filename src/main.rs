extern crate winapi;

use std::ffi::CStr;
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::psapi::{EnumProcesses, GetProcessImageFileNameA};
use winapi::um::sysinfoapi::GlobalMemoryStatusEx;
use winapi::um::sysinfoapi::MEMORYSTATUSEX;
use winapi::um::winnt::{PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

use serde::Deserialize;
use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use std::process::exit;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Deserialize)]
struct Config {
    damon: DamonConfig,
}

#[derive(Deserialize)]
struct DamonConfig {
    server_path: String,
    server_cli_process_name: String,
    memory_thresholds: u64,
}

fn main() {
    let config: Config = read_config("config.toml").expect("Failed to read configuration file.");
    let damon_config = config.damon;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    while running.load(Ordering::SeqCst) {
        if is_process_running(&damon_config.server_cli_process_name) {
            if let Some(available_memory_mb) = get_available_memory() {
                if available_memory_mb < damon_config.memory_thresholds {
                    broadcast_server_restart();
                    execute_commands_concurrently();
                }
            }
        } else {
            thread::sleep(Duration::from_secs(3));
            if let Err(e) = start_process(&damon_config.server_path) {
                eprintln!("Failed to start process: {}", e);
            }
        }

        thread::sleep(Duration::from_secs(1));
    }

    println!("Exiting...");
    exit(0);
}

fn read_config<T: AsRef<std::path::Path>>(
    relative_path: T,
) -> Result<Config, Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;
    let config_path = current_dir.join(relative_path);
    let config_str = fs::read_to_string(config_path)?;
    let config = toml::from_str(&config_str)?;
    Ok(config)
}

fn is_process_running(process_name: &str) -> bool {
    let mut process_ids = [0u32; 1024];
    let mut bytes_returned = 0;

    unsafe {
        if EnumProcesses(
            process_ids.as_mut_ptr(),
            std::mem::size_of_val(&process_ids) as u32,
            &mut bytes_returned,
        ) == 0
        {
            return false;
        }

        let num_processes = bytes_returned / std::mem::size_of::<u32>() as u32;
        for i in 0..num_processes as usize {
            let process_id = process_ids[i];
            let h_process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, process_id);

            if h_process != INVALID_HANDLE_VALUE {
                let mut process_image_name = [0i8; 260];
                if GetProcessImageFileNameA(
                    h_process,
                    process_image_name.as_mut_ptr(),
                    process_image_name.len() as u32,
                ) > 0
                {
                    let process_image_name = CStr::from_ptr(process_image_name.as_ptr())
                        .to_string_lossy()
                        .into_owned();
                    if process_image_name.contains(process_name) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn get_available_memory() -> Option<u64> {
    unsafe {
        let mut mem_status: MEMORYSTATUSEX = std::mem::zeroed();
        mem_status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;

        if GlobalMemoryStatusEx(&mut mem_status) != 0 {
            let available_memory_mb = mem_status.ullAvailPhys / 1024 / 1024;
            println!("Available memory: {} MB", available_memory_mb);
            return Some(available_memory_mb as u64);
        }
    }
    None
}

fn broadcast_server_restart() {
    let message = "server_will_restart_in_60_seconds.";
    let commands = vec![message; 3];

    for command in commands {
        thread::spawn(move || {
            let _ = Command::new("bin/palworld_rcon")
                .arg("bcast")
                .arg(command)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("Failed to execute broadcast command");
        });
    }
}

fn execute_commands_concurrently() {
    let commands = vec!["save", "q"];
    
    for command in commands {
        let _ = Command::new("bin/palworld_rcon")
            .arg(command)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| eprintln!("Failed to execute command '{}': {}", command, e));
    }
}

fn start_process(executable_path: &str) -> std::io::Result<()> {
    Command::new(executable_path)
        .arg("-useperfthreads")
        .arg("-NoAsyncLoadingThread")
        .arg("-UseMultithreadForDS")
        .spawn()
        .map(|_| ())
}

