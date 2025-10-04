use rtsp_parser::RtspServer;
use std::{fs::File, io::Read};
use std::thread;
use std::time::Duration;

fn main() {
    let mut server =  RtspServer::new("127.0.0.1:8554");
    server.add_stream("live");
    let mut file = match File::open("./test.h264") {
        Ok(f) => f,
        Err(_) => {
            println!("File could not open");
            return;
        }
    };

    let mut buffer: Vec<u8> = Vec::new();
    let _ = file.read_to_end(&mut buffer);
    println!("Size of buffer {}", buffer.len());
    let mut remaining = &buffer[..];

    loop {
        match extract_nal(remaining) {
            Some((nal_buf, is_last)) => {
                println!("Nal found with size : {}", nal_buf.len());
                if server.send_frame_to_stream("live" ,nal_buf) {
                    thread::sleep(Duration::from_millis(33));
                    remaining = &remaining[nal_buf.len()..];
                } else {
                    continue;
                }

                if is_last {
                    println!("Last nal");
                    remaining = &buffer[..]; // Reset slice to beginning of the file.
                }
            }
            None => {
                println!("No nal found");
                break;
            }
        }
    }

    server.stop();
}


fn extract_nal(input_buffer: &[u8]) -> Option<(&[u8], bool)> {
    const MAX_START_CODE_LENGTH: usize = 4;

    let mut is_start_found: bool = false;
    let mut is_end_found: bool = false;
    let mut start_code: usize = 0;
    let mut nal_start_index: usize = 0;
    let mut nal_end_index: usize = 0;

    // Find the first nal unit.
    for index in 0..input_buffer.len() - MAX_START_CODE_LENGTH {
        if input_buffer[index] == 0 && input_buffer[index + 1] == 0 && input_buffer[index + 2] == 1
        {
            start_code = 3;
        } else if input_buffer[index] == 0
            && input_buffer[index + 1] == 0
            && input_buffer[index + 2] == 0
            && input_buffer[index + 3] == 1
        {
            start_code = 4;
        } else {
            continue;
        }
        nal_start_index = index; // Keep start code because library expects Nals with start code.
        is_start_found = true;
        break;
    }

    // If there is no start, no need to look for next one.
    if !is_start_found {
        return None;
    }

    // Find second Nal unit.
    for index in nal_start_index + start_code..input_buffer.len() - MAX_START_CODE_LENGTH {
        if input_buffer[index] == 0 && input_buffer[index + 1] == 0 && input_buffer[index + 2] == 1
        {
            // Check if we found a valid nal.
            is_end_found = true;
            nal_end_index = index;
            break;
        } else if input_buffer[index] == 0
            && input_buffer[index + 1] == 0
            && input_buffer[index + 2] == 0
            && input_buffer[index + 3] == 1
        {
            // Check if we found a valid nal.
            is_end_found = true;
            nal_end_index = index;
            break;
        }
    }

    if is_start_found && is_end_found {
        return Some((&input_buffer[nal_start_index..nal_end_index], false));
    } else if is_start_found && !is_end_found {
        return Some((&input_buffer[nal_start_index..], true));
    }

    return None;
}