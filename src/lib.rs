use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,Mutex
};
use std::thread::{self};
use std::collections::HashMap;
use std::time::Duration;


pub struct RtspServer {
    tcp_server: TcpListener,
    is_thread_running: AtomicBool,
    streams: Mutex<Vec<String>>
}

enum RtspMethod {
    Options,
    Describe,
    Setup,
    Play,
    Pause,
    Teardown,
    Announce,
    Record,
    Redirect,
    Unknown(String), // fallback for unrecognized methods
}

impl std::str::FromStr for RtspMethod {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "OPTIONS"  => Ok(RtspMethod::Options),
            "DESCRIBE" => Ok(RtspMethod::Describe),
            "SETUP"    => Ok(RtspMethod::Setup),
            "PLAY"     => Ok(RtspMethod::Play),
            "PAUSE"    => Ok(RtspMethod::Pause),
            "TEARDOWN" => Ok(RtspMethod::Teardown),
            "ANNOUNCE" => Ok(RtspMethod::Announce),
            "RECORD"   => Ok(RtspMethod::Record),
            "REDIRECT" => Ok(RtspMethod::Redirect),
            _          => Ok(RtspMethod::Unknown(s.to_string())),
        }
    }
}

struct RtspRequest {
    method: RtspMethod,
    uri: String,
    version: String,
    cseq: u32,
    headers: HashMap<String, String>,
    body: Option<String>,
}
impl std::str::FromStr for RtspRequest {
    type Err = String; // could be a custom error type

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut lines = s.split("\r\n");

        // Parse request line
        let request_line = lines.next().ok_or("Empty request")?;
        let mut parts = request_line.split_whitespace();
        let method_str = parts.next().ok_or("Missing method")?;
        let method = method_str.parse().map_err(|_| "Invalid method")?;
        let uri = parts.next().ok_or("Missing URI")?.to_string();
        let version = parts.next().ok_or("Missing version")?.to_string();

        // Parse headers
        let mut headers = HashMap::new();
        for line in &mut lines {
            if line.is_empty() { break; } // empty line = end of headers
            if let Some((key, value)) = line.split_once(":") {
                headers.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        let cseq: u32 = headers.get("CSeq").unwrap().parse().unwrap();

        // Parse body
        let body: String = lines.collect::<Vec<_>>().join("\r\n");
        let body = if body.is_empty() { None } else { Some(body) };

        Ok(RtspRequest { method, uri, version, cseq, headers, body })
    }
}

impl RtspServer {

    pub fn new(address: &str) -> Arc<Self> {
        let tcp_server = TcpListener::bind(address).unwrap();
        
        let myself = Arc::new(Self { 
            tcp_server: tcp_server,
            is_thread_running: AtomicBool::new(true),
            streams: Mutex::new(Vec::new()), 
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

    pub fn add_stream(&self, stream_name: &str) {
        let mut streams = self.streams.lock().unwrap();
        streams.push(stream_name.to_string());
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