use async_tungstenite::WebSocketStream;
use futures::future;
use futures_util::sink::SinkExt;
use smol::{self, block_on, Async, Task};
use std::net::TcpListener;
use std::thread;
use tungstenite::Message;

fn main() {
    println!("Hello World!");
    let cpus = num_cpus::get().max(1);
    for _ in 0..cpus - 1 {
        thread::spawn(move || smol::run(future::pending::<()>()));
    }
    block_on(async move {
        let listener =
            Async::<TcpListener>::bind("127.0.0.1:9001").expect("could not bind to the localhost");
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws_stream: WebSocketStream<_> = async_tungstenite::accept_async(stream)
                .await
                .expect("could not accept new connection");
            Task::spawn(async move {
                loop {
                    println!(
                        "sending message, from threadId: {:?}",
                        thread::current().id()
                    );
                    ws_stream.send(Message::text("sfsdfsdff")).await.unwrap();
                    smol::Timer::after(std::time::Duration::from_secs(2)).await;
                }
            })
            .detach();
        }
    });
}
