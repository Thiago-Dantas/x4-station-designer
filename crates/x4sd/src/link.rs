//! Live link between `x4sd mcp` and a running builder (D-7): a local TCP
//! socket with one JSON line per message. The builder greets with
//! `{"x4sd": <version>}`; each request is an `Op`, each reply
//! `{"ok": <value>}` or `{"err": <message>}`.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::ops::Op;

pub const ADDR: &str = "127.0.0.1:47823";

pub struct Client {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl Client {
    /// Connects to a running builder, or `None` if there is none.
    pub fn connect() -> Option<Client> {
        let addr: SocketAddr = ADDR.parse().ok()?;
        let stream = TcpStream::connect_timeout(&addr, Duration::from_millis(200)).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
        let mut reader = BufReader::new(stream.try_clone().ok()?);
        let mut hello = String::new();
        reader.read_line(&mut hello).ok()?;
        serde_json::from_str::<Value>(&hello).ok()?.get("x4sd")?;
        // Operations can take a moment (catalog queries); greet fast, answer slower.
        stream.set_read_timeout(Some(Duration::from_secs(30))).ok()?;
        Some(Client { reader, writer: stream })
    }

    /// Runs `op` in the builder. The outer error means the connection broke.
    pub fn send(&mut self, op: &Op) -> std::io::Result<Result<Value, String>> {
        let mut line = serde_json::to_string(op)?;
        line.push('\n');
        self.writer.write_all(line.as_bytes())?;
        let mut reply = String::new();
        if self.reader.read_line(&mut reply)? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        let reply: Value = serde_json::from_str(&reply)?;
        Ok(match (reply.get("ok"), reply.get("err")) {
            (Some(v), _) => Ok(v.clone()),
            (_, Some(e)) => Err(e.as_str().unwrap_or("builder error").to_owned()),
            _ => Err("malformed reply from the builder".to_owned()),
        })
    }
}

/// Listens for agents in a background thread; `handler` runs each operation.
/// Fails if another builder already holds the port.
pub fn serve(handler: Arc<dyn Fn(Op) -> Result<Value, String> + Send + Sync>) -> std::io::Result<()> {
    let listener = TcpListener::bind(ADDR)?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let handler = handler.clone();
            std::thread::spawn(move || {
                let _ = serve_client(stream, &*handler);
            });
        }
    });
    Ok(())
}

fn serve_client(stream: TcpStream, handler: &(dyn Fn(Op) -> Result<Value, String> + Send + Sync)) -> std::io::Result<()> {
    let mut writer = stream.try_clone()?;
    writeln!(writer, "{}", json!({"x4sd": env!("CARGO_PKG_VERSION")}))?;
    for line in BufReader::new(stream).lines() {
        let reply = match serde_json::from_str::<Op>(&line?) {
            Ok(op) => match handler(op) {
                Ok(v) => json!({"ok": v}),
                Err(e) => json!({"err": e}),
            },
            Err(e) => json!({"err": format!("bad request: {e}")}),
        };
        writeln!(writer, "{reply}")?;
    }
    Ok(())
}
