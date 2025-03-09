use serde::{Deserialize, Serialize};
use std::time::Instant;
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[derive(Clone, Serialize, Deserialize, Debug)]
struct ClientSetupParams {
    seed: [u8; chalamet_pir::SEED_BYTE_LEN],
    hint: Vec<u8>,
    filter: Vec<u8>,
}

#[tokio::main]
async fn main() {
    // Connect to the server.
    let mut stream = TcpStream::connect("127.0.0.1:7878")
        .await
        .expect("Unable to connect to server");

    println!("Connected to server at 127.0.0.1:7878");

    // Create a buffered reader for stdin.
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut input = String::new();

    loop {
        input.clear();
        println!("Type a message and press Enter (or type 'quit' to exit):");

        // Read a line from the standard input.
        match reader.read_line(&mut input).await {
            Ok(0) => {
                // End-of-file (EOF) reached.
                println!("EOF reached, exiting.");
                break;
            }
            Ok(_) => {
                let trimmed = input.trim_end();
                if trimmed.eq_ignore_ascii_case("quit") {
                    println!("Exiting.");
                    break;
                }

                // Write the trimmed input to the server.
                if let Err(e) = stream.write_all(trimmed.as_bytes()).await {
                    eprintln!("Failed to write to stream: {}", e);
                    break;
                }

                // Read the response from the server.
                let start_tm = Instant::now();

                let mut len_bytes = [0u8; std::mem::size_of::<u64>()];
                match stream.read_exact(&mut len_bytes).await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Failed to read length from stream: {}", e);
                        break;
                    }
                }

                let buffer_len = u64::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
                let mut buffer = vec![0; buffer_len];
                let mut bytes_read = 0;

                loop {
                    match stream.read(&mut buffer[bytes_read..]).await {
                        Ok(0) => break,
                        Ok(n) => bytes_read += n,
                        Err(e) => {
                            eprintln!("Failed to read from stream: {}", e);
                            break;
                        }
                    }
                    if bytes_read > 0 && bytes_read >= buffer.len() {
                        break;
                    }
                }

                println!("Read : {}B, in {:?}", bytes_read, start_tm.elapsed());

                let client_setup_params: ClientSetupParams =
                    serde_json::from_slice(&buffer).unwrap();
            }
            Err(e) => {
                eprintln!("Failed to read from stdin: {}", e);
                break;
            }
        }
    }
}
