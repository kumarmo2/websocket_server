use async_tungstenite::WebSocketStream;
use chat_common_types::events::ClientEventQueueNameWrapper;
use futures::future;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use manager::{
    BasicAckOptions, BasicConsumeOptions, ChannelManager, FieldTable, Pool, RabbitMqManager,
};
use smol::{self, block_on, Async, Task};
use std::net::TcpListener;
use std::net::TcpStream;
use std::thread;
use tungstenite::Message;

fn main() {
    println!("Hello World!");
    // create thread pool.
    let cpus = num_cpus::get().max(1);
    for _ in 0..cpus {
        thread::spawn(move || smol::run(future::pending::<()>()));
    }
    let addr = "amqp://guest:guest@127.0.0.1:5672/%2f";
    //TODO: update RabbitMqManager to accept max connections/channgels.
    let rabbit = RabbitMqManager::new(addr);

    block_on(async move {
        let listener =
            Async::<TcpListener>::bind("127.0.0.1:9001").expect("could not bind to the localhost");
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            // TODO: Add authorization
            let ws_stream: WebSocketStream<_> = async_tungstenite::accept_async(stream)
                .await
                .expect("could not accept new connection");
            let pool = rabbit.get_channel_pool();
            Task::spawn(async move {
                handle_connection(ws_stream, pool).await;
            })
            .detach();
        }
    });
}

async fn handle_connection(
    mut ws_stream: WebSocketStream<Async<TcpStream>>,
    channel_pool: Pool<ChannelManager>,
) {
    let queue_name;
    // loop while we don't receive the queue_name.
    loop {
        println!(
            "sending message, from threadId: {:?}",
            thread::current().id()
        );
        // TODO: make error handling(communication of queue name)
        // more robust.
        let msg: tungstenite::Message = ws_stream
            .next()
            .await
            .expect("could not read message")
            .expect("some error");
        if msg.is_text() {
            match serde_json::from_str::<ClientEventQueueNameWrapper>(&msg.into_text().unwrap()) {
                Ok(queue_wrapper) => {
                    println!("queue name: {}", queue_wrapper.queue_name);
                    queue_name = queue_wrapper.queue_name;
                    break;
                }
                Err(reason) => {
                    println!("could not deserialize message: {}", reason);
                }
            }
        }
    }
    // TODO: Here, there might be a scaling problem, as a single channel is
    // reserved for a client. Need to test this.

    // TODO: need to write a resilient logic and a proper Refactoring.
    let (mut outgoing, _) = ws_stream.split();
    loop {
        let channel = channel_pool.get().unwrap();
        println!("will consume");
        let consumer = channel
            .basic_consume(
                &queue_name,
                "message_service",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .unwrap();
        for delivery in consumer {
            println!("message received",);
            if let Ok(delivery) = delivery {
                let m = Message::text(std::str::from_utf8(&delivery.data).expect("sdfsdf"));
                match outgoing.send(m).await {
                    Ok(_) => {
                        channel
                            .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                            .await
                            .expect("error while acknowledging the message");
                    }
                    Err(_) => println!("failed forwarding message"),
                }
            }
        }
    }
}
