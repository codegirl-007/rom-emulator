use futures::{SinkExt, StreamExt};
use log::{error, info};
use std::env;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};

// this file is overly commented because I'm learning Rust while I program. I want just want to
// thnk aloud.
#[tokio::main]
async fn main() {
    // initialize logger
    env_logger::init();

    // retrieve the iterator from command line args passed in, extract second item, if nth returns
    // a value, extra it or else if it returns nothing, use the given ip address
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    // SocketAddr is used to represent an IP address and port combo. .parse attempts to convert the
    // string to a SocketAddr. If successful, returns a Result<SocketAddr, _>. expect() will
    // terminate the program and print a message if parse returns an error. If parse is successful,
    // expect() will unwrap the SockAddr from the Result and assign it to addr.
    let addr: SocketAddr = addr.parse().expect("Invalid Address");

    // attempt to bind the tcplistner to an address. await indicates that this is async.
    // TCPListener::bind will not block the current thread but instead yield control until the
    // binding operation completes. If bind fails, it will return an Err.
    //
    // The await keyword here tells the runtime that the function will wait for TCPListener::bind
    // to complete and will not block the thread. Instead, the runtime can schedule other tasks to
    // run on the same thread while original operation waits for it's completion.
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");

    // info! is a macro that logs a message at the info level.
    info!("Listening on: {}", addr);

    // This is a pattern matching loop. It will continuausly process connections as long as the
    // await returns Ok. If an error occurs, the loop will stop.
    //
    // The accept method waits asynchronously for an incoming tCP connection.
    // When a client connects, it returns a Result containing TcpStream and SocketAddr. TcpStream
    // is a handle for the client connection, allow for read and writes.
    while let Ok((stream, _)) = listener.accept().await {
        // tokio::spawn will spawn a new async task to handle the connection. This allows for
        // multiple clients to connect and to be processed concurrently. Each task runs
        // independently and does not block server loop.
        tokio::spawn(handle_connection(stream));
    }
}

async fn handle_connection(stream: TcpStream) {
    // perform the handshake on the given TCP stream
    // The match expression is used to handle the result of the accept_async function
    // accept_async will return a Result that is either an Ok(ws) which will hand us the websocket
    // connection or an error message.
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("Error during the websocket handshake: {}", e);
            return;
        }
    };

    // split websocket stream into sender and receiver
    let (mut sender, mut receiver) = ws_stream.split();

    // handle incoming websocket messages and reverse the string that comes in and then send it
    // back. Why? Because.
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let reversed = text.chars().rev().collect::<String>();
                // .into converts the types when possible. The Message::Text variant expects String
                // but reversed might not match that exact implementation. Libraries like
                // tokio-tungstenite may require into.
                if let Err(e) = sender.send(Message::Text(reversed.into())).await {
                    error!("Error sending message: {}", e);
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => (),
            Err(e) => {
                error!("Error processing message: {}", e);
            }
        }
    }
}
