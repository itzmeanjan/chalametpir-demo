use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

pub const ARITY: u32 = 3; // It can also be 4.

#[derive(Serialize, Deserialize)]
pub struct ClientSetupParams {
    pub seed: [u8; chalamet_pir::SEED_BYTE_LEN],
    pub hint: Vec<u8>,
    pub filter: Vec<u8>,
}

pub async fn read_message_byte_length(stream: &mut TcpStream) -> Option<usize> {
    stream.read_u64_le().await.ok().map(|v| v as usize)
}

pub async fn read_message(stream: &mut TcpStream, msg_byte_len: usize) -> Vec<u8> {
    let mut msg_bytes = vec![0u8; msg_byte_len];
    let mut bytes_read = 0;

    loop {
        match stream.read(&mut msg_bytes[bytes_read..]).await {
            Ok(0) => break,
            Ok(n) => bytes_read += n,
            Err(e) => {
                eprintln!("❌ Read {}B from client stream, failed to read any further: {}", bytes_read, e);
                break;
            }
        }
        if bytes_read == msg_byte_len {
            break;
        }
    }

    msg_bytes
}
