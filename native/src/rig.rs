use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};
#[derive(Debug, Clone)]
pub struct Radio {
    pub mhz: f64,
    pub mode: String,
}
pub fn read_radio(address: SocketAddr) -> Result<Radio, String> {
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(2)).map_err(|_| {
            "SDR++ is not reachable. Start Rigctl Server on localhost:4532 with Radio selected."
                .to_string()
        })?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream.write_all(b"f\nm\n").map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or("SDR++ reply timed out.")?;
        stream
            .set_read_timeout(Some(left))
            .map_err(|e| e.to_string())?;
        let n = stream
            .read(&mut chunk)
            .map_err(|e| format!("Could not read SDR++: {e}"))?;
        if n == 0 {
            return Err("SDR++ closed before sending a complete reply.".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() > 8192 {
            return Err("SDR++ reply is too large.".into());
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| "Invalid SDR++ reply.")?;
        let lines: Vec<_> = text
            .split('\n')
            .take(text.bytes().filter(|b| *b == b'\n').count())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(e) = lines.iter().find(|s| s.starts_with("RPRT")) {
            return Err(format!("SDR++ returned {e}"));
        }
        if lines.len() >= 3 {
            let hz = lines[0]
                .parse::<f64>()
                .map_err(|_| "Invalid SDR++ frequency.")?;
            let bw = lines[2]
                .parse::<f64>()
                .map_err(|_| "Invalid SDR++ bandwidth.")?;
            if !hz.is_finite()
                || hz <= 0.0
                || !bw.is_finite()
                || !lines[1]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err("Invalid SDR++ reply.".into());
            }
            return Ok(Radio {
                mhz: hz / 1e6,
                mode: lines[1].into(),
            });
        }
    }
}
