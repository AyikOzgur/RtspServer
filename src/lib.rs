mod rtsp_request;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,Mutex
};
use std::thread;
use std::collections::HashMap;


use rtp_transceive::H264RtpPusher;

use crate::rtsp_request::{RtspMethod, RtspRequest};


pub struct RtspServer {
    tcp_server: TcpListener,
    is_thread_running: AtomicBool,
    streams: Mutex<Vec<String>>,
    rtp_pushers: Mutex<HashMap<String, H264RtpPusher>>
}

impl RtspServer {

    pub fn new(address: &str) -> Arc<Self> {
        let tcp_server = TcpListener::bind(address).unwrap();
        
        let myself = Arc::new(Self { 
            tcp_server: tcp_server,
            is_thread_running: AtomicBool::new(true),
            streams: Mutex::new(Vec::new()), 
            rtp_pushers: Mutex::new(HashMap::new())
        });

        let my_clone: Arc<RtspServer> = Arc::clone(&myself);

        thread::spawn(move || {
            RtspServer::connection_accept(my_clone);
        });

        myself
    }

    fn connection_accept(self_arc: Arc<Self>) {
        // Listen tcp sockets and jjust print the address of clients that tries to connect for now
        while self_arc.is_thread_running.load(Ordering::Relaxed) {
            let stream = self_arc.tcp_server.accept();
            match stream {
                Ok((stream, address)) => {
                    println!("Client address {}", address);
                    let server_clone = Arc::clone(&self_arc);
                    thread::spawn(move || {
                        server_clone.handle_client(stream);
                    });
                    }
                Err(_) => {}
            }
        }
    }

    fn handle_client(&self, mut client: TcpStream) {
        let mut incoming_buffer: [u8; 2048] = [0u8; 2048];
        while self.is_thread_running.load(Ordering::Relaxed) {
            // Here we can read until any request comes
            match client.read(&mut incoming_buffer) {
                Ok(0) => { 
                    break; // Finish the thread because client is disconnected.
                }
                Ok(n) => {
                    if let Ok (message) = std::str::from_utf8(&incoming_buffer[..n]) {
                        let response =  self.handle_request(message);
                        let _ = client.write_all(response.as_bytes());
                    } else {
                        println!("Invalid message from client[{}]", client.peer_addr().unwrap().port());
                    }
                }
                Err(_) => {}
            }
        }
    }

    fn handle_request(&self, buffer: &str) -> String {
        println!("Request : \n {}", buffer);
        let request: RtspRequest = buffer.parse().unwrap();

        // Check if we have such stream.
        let stream_name = request.uri.rsplit('/').next().unwrap_or("");
        let streams = self.streams.lock().unwrap();
        if !streams.contains(&stream_name.to_string()) {
            return format!("RTSP/1.0 400 Bad Request \r\nCSeq: {}\r\n", request.cseq);
        }

        let response_body: String  = match request.method {
            RtspMethod::Options => {
                "Public: OPTIONS, DESCRIBE, SETUP, TEARDOWN, PLAY, PAUSE\r\n\r\n".to_string()
            }
            RtspMethod::Describe => {
                let sdp = 
                    "v=0\r\n\
                    o=- 0 0 IN IP4 127.0.0.1\r\n\
                    s=Example Stream\r\n\
                    t=0 0\r\n\
                    m=video 0 RTP/AVP 96\r\n\
                    a=rtpmap:96 H264/90000";

                format!("Content-Type: application/sdp\r\nContent-Length: {}\r\n\r\n{}", sdp.len(), sdp)
            }
            RtspMethod::Setup => {
                let transport = request.headers.get("Transport").unwrap();

                // Here parse the transport message and initialize the RTP pusher. Initially support only UDP maybe.

                // You can append server_port if you like
                let transport_header = format!("{};server_port=6000-6001", transport);

                let rtp_port = Self::extract_rtp_port(&transport).unwrap();
                println!("Port: {rtp_port}");
                let destination_address = format!("127.0.0.1:{}", rtp_port);

                let rtp_pusher = H264RtpPusher::new(&destination_address);

                let mut rtp_pushers = self.rtp_pushers.lock().unwrap();
                rtp_pushers.insert(stream_name.to_string(), rtp_pusher);

                format!("Session: 47112344\r\nTransport: {}\r\n\r\n", transport)
            }
            RtspMethod::Play => {
                // Maybe here enable RTP pusher.
                format!("Session: 47112344\r\n\r\n")
            }
            _ => {
                "".to_string()
            }
        };

        if response_body.len() == 0 {
            return format!("RTSP/1.0 400 Bad Request \r\nCSeq: {}\r\n\r\n", request.cseq);
        }

        let mut full_response = String::from(format!("RTSP/1.0 200 OK\r\nCSeq: {}\r\n", request.cseq));
        full_response.push_str(&response_body);
        println!("Full response : \n {full_response}");
        full_response
    }

    fn extract_rtp_port(header: &str) -> Option<u16> {
        header
            .split(';')
            .find_map(|part| part.trim().strip_prefix("client_port="))
            .and_then(|ports| ports.split('-').next())
            .and_then(|rtp| rtp.parse::<u16>().ok())
    }


    pub fn add_stream(&self, stream_name: &str) {
        let mut streams = self.streams.lock().unwrap();
        streams.push(stream_name.to_string());
    }

    pub fn send_frame_to_stream(&self, stream_name: &str, frame_buffer: &[u8]) -> bool {
        let mut pushers = self.rtp_pushers.lock().unwrap();
        let pusher  = pushers.get_mut(stream_name);
        match pusher {
            Some (push) => {
                push.send_frame(frame_buffer);
            }
            _ => {
                return false;
            }
        }

        false
    }

    pub fn stop(&self) {
        self.is_thread_running.store(false, Ordering::Relaxed);
    }
}

impl Drop for RtspServer {
    fn drop(&mut self) {
        self.stop();
    }
}