//! Serves rendered preview MP4 files to the webview.
//!
//! On Linux, `convertFileSrc` uses `asset://`, and WebKit's media element does
//! not play that scheme. A loopback server avoids putting source media on the
//! network: only `*.mp4` files whose names are 64 hex characters are readable.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

static BASE: OnceLock<String> = OnceLock::new();

pub fn base_url() -> Option<&'static str> {
    BASE.get().map(String::as_str)
}

pub fn serve(root: PathBuf) -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let _ = BASE.set(format!("http://127.0.0.1:{port}"));
    thread::Builder::new()
        .name("pulsecut-preview".into())
        .spawn(move || {
            for conn in listener.incoming().flatten() {
                let root = root.clone();
                thread::spawn(move || {
                    let _ = handle_client(conn, &root);
                });
            }
        })?;
    Ok(port)
}

pub fn file_name_ok(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".mp4") else {
        return false;
    };
    stem.len() == 64 && stem.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn handle_client(mut stream: TcpStream, root: &PathBuf) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = [0u8; 8192];
    let mut filled = 0;
    let header_end = loop {
        if filled == buf.len() {
            return write_status(&mut stream, 400, b"Bad Request");
        }
        let read = stream.read(&mut buf[filled..])?;
        if read == 0 {
            return Ok(());
        }
        filled += read;
        if let Some(pos) = find_header_end(&buf[..filled]) {
            break pos;
        }
    };
    let header = String::from_utf8_lossy(&buf[..header_end]);
    let mut lines = header.split("\r\n");
    let Some(request) = lines.next() else {
        return write_status(&mut stream, 400, b"Bad Request");
    };
    let mut parts = request.split(' ');
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    if method != "GET" && method != "HEAD" {
        return write_status(&mut stream, 405, b"Method Not Allowed");
    }
    let path = target.split('?').next().unwrap_or("");
    let name = path.strip_prefix('/').unwrap_or("");
    if name.contains('/') || !file_name_ok(name) {
        return write_status(&mut stream, 404, b"Not Found");
    }
    let file_path = root.join(name);
    let mut file = match File::open(&file_path) {
        Ok(file) => file,
        Err(_) => return write_status(&mut stream, 404, b"Not Found"),
    };
    let len = file.metadata()?.len();
    let range = lines.find_map(|line| {
        let rest = line
            .strip_prefix("Range:")
            .or_else(|| line.strip_prefix("range:"))?;
        parse_range(rest.trim(), len)
    });
    let (status, start, end) = match range {
        Some(Ok((start, end))) => (206, start, end),
        Some(Err(())) => return write_status(&mut stream, 416, b"Range Not Satisfiable"),
        None => (200, 0, len.saturating_sub(1)),
    };
    if len == 0 {
        return write_status(&mut stream, 200, b"");
    }
    let body_len = end - start + 1;
    let mut response = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: video/mp4\r\nAccept-Ranges: bytes\r\nContent-Length: {body_len}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n",
        if status == 206 { "Partial Content" } else { "OK" }
    );
    if status == 206 {
        response.push_str(&format!("Content-Range: bytes {start}-{end}/{len}\r\n"));
    }
    response.push_str("\r\n");
    stream.write_all(response.as_bytes())?;
    if method == "HEAD" {
        return Ok(());
    }
    file.seek(SeekFrom::Start(start))?;
    let mut left = body_len;
    let mut chunk = [0u8; 64 * 1024];
    while left > 0 {
        let want = chunk.len().min(left as usize);
        let read = file.read(&mut chunk[..want])?;
        if read == 0 {
            break;
        }
        stream.write_all(&chunk[..read])?;
        left -= read as u64;
    }
    Ok(())
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_range(value: &str, len: u64) -> Option<Result<(u64, u64), ()>> {
    let rest = value.strip_prefix("bytes=")?;
    let (start_text, end_text) = rest.split_once('-')?;
    if len == 0 {
        return Some(Err(()));
    }
    if start_text.is_empty() {
        let suffix: u64 = end_text.parse().ok()?;
        if suffix == 0 {
            return Some(Err(()));
        }
        let start = len.saturating_sub(suffix);
        return Some(Ok((start, len - 1)));
    }
    let start: u64 = start_text.parse().ok()?;
    if start >= len {
        return Some(Err(()));
    }
    let end = if end_text.is_empty() {
        len - 1
    } else {
        let end: u64 = end_text.parse().ok()?;
        end.min(len - 1)
    };
    if end < start {
        return Some(Err(()));
    }
    Some(Ok((start, end)))
}

fn write_status(stream: &mut TcpStream, status: u16, body: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    #[test]
    fn only_hash_named_mp4s_are_public() {
        let name = "780f99565a620e71b4ddf2e4ff7a4e54a5782c2003ed43c75c9206f4e1f537a5.mp4";
        assert!(file_name_ok(name));
        assert!(!file_name_ok("../secret.mp4"));
        assert!(!file_name_ok("notes.mp4"));
        assert!(!file_name_ok(&format!("{name}/extra")));
    }

    #[test]
    fn serves_a_range_from_the_preview_directory() {
        let root = std::env::temp_dir().join(format!("pulsecut-http-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let name = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.mp4";
        std::fs::write(root.join(name), b"0123456789").unwrap();
        let port = serve(root.clone()).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .write_all(
                format!("GET /{name} HTTP/1.1\r\nRange: bytes=2-5\r\nHost: 127.0.0.1\r\n\r\n")
                    .as_bytes(),
            )
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        let text = String::from_utf8_lossy(&response);
        assert!(text.starts_with("HTTP/1.1 206"), "{text}");
        assert!(text.contains("Content-Range: bytes 2-5/10"));
        assert!(text.ends_with("2345"), "{text}");
        let mut denied = TcpStream::connect(("127.0.0.1", port)).unwrap();
        denied
            .write_all(b"GET /../Cargo.toml HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();
        let mut body = Vec::new();
        denied.read_to_end(&mut body).unwrap();
        assert!(String::from_utf8_lossy(&body).starts_with("HTTP/1.1 404"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
