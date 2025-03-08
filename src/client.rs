use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

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

                // Write the input to the server.
                if let Err(e) = stream.write_all(input.as_bytes()).await {
                    eprintln!("Failed to write to stream: {}", e);
                    break;
                }

                // Read the response from the server.
                let mut buffer = vec![0; 512];
                match stream.read(&mut buffer).await {
                    Ok(0) => {
                        println!("Server closed the connection.");
                        break;
                    }
                    Ok(n) => {
                        let response = String::from_utf8_lossy(&buffer[..n]);
                        println!("Received echo: {}", response);
                    }
                    Err(e) => {
                        eprintln!("Failed to read from stream: {}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to read from stdin: {}", e);
                break;
            }
        }
    }
}
