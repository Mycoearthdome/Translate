extern crate libc;
extern crate nix;
extern crate sysinfo;

const LIME_MODULE: &str = "/home/jordan/Documents/LiME/lime-6.9.3-tsurugi.ko";
const MEMORY_FILE: &str = "memfile";
const PAGE_SIZE: u64 = 4096;

use std::fs::exists;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::{BufRead, BufReader};
use std::process::Command;

fn launcher() -> (Vec<String>, Vec<String>) {
    let ls = Command::new("ls")
        .arg("-l")
        .arg("/proc")
        .output()
        .expect("Failed to execute strace");
    let ls_reader = BufReader::new(ls.stdout.as_slice());

    let mut running_pids: Vec<String> = Vec::new();
    let mut stacks: Vec<String> = Vec::new();

    for line in ls_reader.lines() {
        match line {
            Ok(line) => {
                if line.contains("dr-") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    let pid = parts[parts.len() - 1];
                    if exists(format!("/proc/{}/maps", pid))
                        .expect("Failed to check if file exists")
                    {
                        let cat = Command::new("cat")
                            .arg(format!("/proc/{}/maps", pid))
                            .output()
                            .expect("Failed to execute strace");
                        let maps_reader = BufReader::new(cat.stdout.as_slice());
                        for line2 in maps_reader.lines() {
                            match line2 {
                                Ok(line2) => {
                                    if line2.contains("[stack]") {
                                        running_pids.push(pid.to_string());
                                        let line2_clone = line2.clone();
                                        let parts2: Vec<&str> =
                                            line2_clone.split_whitespace().collect();
                                        let reserved_stack = parts2[0].to_string();
                                        stacks.push(reserved_stack);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error reading line: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading line: {}", e); // Print error
                break; // Error reading line
            }
        }
    }
    (running_pids, stacks)
}

fn read_memory_dump(file_path: &str, offset: u64, length: usize) -> std::io::Result<String> {
    let mut file = File::open(file_path).map_err(|e| {
        eprintln!("Failed to open file {}: {}", file_path, e);
        e
    })?;

    // Check the file size
    let metadata = file.metadata()?;
    let file_size = metadata.len();

    // Validate the offset
    if offset >= file_size {
        eprintln!(
            "Offset {} is out of bounds for file size {}",
            offset, file_size
        );
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Offset out of bounds",
        ));
    }

    // Seek to the specified offset
    file.seek(SeekFrom::Start(offset)).map_err(|e| {
        eprintln!("Failed to seek to offset {}: {}", offset, e);
        e
    })?;

    let mut buffer = vec![0; length];
    file.read_exact(&mut buffer)?;
    let string = String::from_utf8_lossy(&buffer);
    Ok(string.to_string())
}

fn dump_raw_ram(path_to_lime_module: String, memfile: String) {
    println!("Dumping raw RAM to file: {}", memfile);
    println!("insmod {} path={} format=raw", path_to_lime_module, memfile);
    if exists(path_to_lime_module.clone()).unwrap() {
        let mut child = Command::new("insmod")
            .arg(path_to_lime_module)
            .arg(format!("path={}", memfile))
            .arg("format=raw")
            .spawn()
            .expect("Failed to execute insmod");

        let _ = child.wait().expect("Child process wasn't running");
    }
}

fn unload_lime_module() {
    let mut child = Command::new("rmmod")
        .arg("lime")
        .spawn()
        .expect("Failed to execute rmmod");

    let _ = child.wait().expect("Child process wasn't running");
}

fn main() -> std::io::Result<()> {
    let (pids, stacks) = launcher();

    // Read the memory maps of the processes
    let mut physical_address;
    let mut stack_index = 0;

    dump_raw_ram(LIME_MODULE.to_string(), MEMORY_FILE.to_string());

    for pid in pids {
        let address_range: Vec<&str> = stacks[stack_index].split("-").collect();
        print!("PID: {} ", pid);

        let start_address: u64 = match u64::from_str_radix(&address_range[0][4..], 16) {
            Ok(va) => va,
            Err(_) => {
                println!("ERROR");
                continue;
            }
        };

        let end_address: u64 = match u64::from_str_radix(&address_range[1][4..], 16) {
            Ok(va) => va,
            Err(_) => {
                println!("ERROR");
                continue;
            }
        };

        let length = (end_address - start_address) as u32;

        physical_address = start_address & !(PAGE_SIZE - 1);

        println!("- Stack: {}", stacks[stack_index]);
        println!(
            "{}",
            read_memory_dump(MEMORY_FILE, physical_address, length as usize)?,
        );

        println!("Physical address (hex): 0x{:x}", physical_address);
        println!("Physical address (dec): {}", physical_address);

        stack_index += 1;
    }

    unload_lime_module();

    Ok(())
}
