use rtsp_parser::RtspServer;

fn main() {
    let mut server =  RtspServer::new("127.0.0.1:8554");
    server.add_stream("live");

    loop {}

    server.stop();
}