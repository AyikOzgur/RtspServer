
use std::collections::HashMap;

pub enum RtspMethod {
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

pub struct RtspRequest {
    pub method: RtspMethod,
    pub uri: String,
    pub version: String,
    pub cseq: u32,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
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