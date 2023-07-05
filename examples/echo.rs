use std::{
    cmp,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};

use async_smux::MuxBuilder;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinSet,
};

const BUF_SIZE: usize = 1024 * 60;
const LARGE_SIZE: usize = 1024 * 1024 * 1024;
const MAX_QUEUE: usize = 128 * 1024;
const COUNT: usize = 100;
const LARGE_FREQ: usize = 10;
const BREAK: u8 = b'\n';

async fn echo_server() {
    let listener = TcpListener::bind("127.0.0.1:12345").await.unwrap();
    let (stream, _) = listener.accept().await.unwrap();

    let (_, mut acceptor, worker) = MuxBuilder::server()
        .with_max_rx_queue(NonZeroUsize::new(MAX_QUEUE).unwrap())
        .with_max_tx_queue(NonZeroUsize::new(MAX_QUEUE).unwrap())
        .with_connection(stream)
        .build();
    tokio::spawn(worker);

    println!("server launched");
    while let Some(mut mux_stream) = acceptor.accept().await {
        let mut buf = [0u8; BUF_SIZE];
        loop {
            let Ok(mut length) = mux_stream.read(&mut buf).await else {
                break;
            };
            if length == 0 {
                break;
            }
            for (i, c) in buf[..length].iter().enumerate() {
                if *c == BREAK {
                    length = i + 1;
                }
            }
            if buf[0] == b'@' {
                let s = String::from_utf8_lossy(&buf[7..16]);
                println!("                          {} Received, now sending", s);
            }
            match mux_stream.write(&buf[..length]).await {
                Ok(written) => {
                    if written < length {
                        println!("Write not complete");
                        break;
                    }
                }
                Err(_err) => {
                    break;
                }
            }
            let _ = mux_stream.flush().await;
        }
        mux_stream.shutdown().await.unwrap();
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tokio::spawn(echo_server());
    tokio::time::sleep(Duration::from_secs(1)).await;

    let stream = TcpStream::connect("127.0.0.1:12345").await.unwrap();
    let (connector, _, worker) = MuxBuilder::client()
        .with_max_rx_queue(NonZeroUsize::new(MAX_QUEUE).unwrap())
        .with_max_tx_queue(NonZeroUsize::new(MAX_QUEUE).unwrap())
        .with_connection(stream)
        .build();
    tokio::spawn(worker);
    let connector = Arc::new(connector);

    println!("Start");

    let mut join_set = JoinSet::new();
    for i in 0..COUNT {
        let connector = connector.clone();
        join_set.spawn(async move {
            let start = Instant::now();
            let mut mux_stream = connector.connect().unwrap();
            let to_write = if i % LARGE_FREQ == 0 {
                let mut v = vec![0; LARGE_SIZE];
                for (i, c) in v.iter_mut().enumerate() {
                    *c = (97 + i % 26) as u8;
                }
                v
            } else {
                Vec::from(format!("@hello {:9}", i))
            };
            let mut buf = [0u8; BUF_SIZE];
            let mut written = 0;
            while written < to_write.len() {
                let write_start = written;
                for i in 0..cmp::min(buf.len(), to_write.len() - written) {
                    buf[i] = to_write[written + i];
                }
                let length = mux_stream.write(&buf[..]).await.unwrap();
                if length == 0 {
                    break;
                }
                mux_stream.flush().await.unwrap();
                written += length;
                if i % LARGE_FREQ != 0 {
                    println!("    {:5} Sent, now waiting", i);
                }
                for i in 0..buf.len() {
                    buf[i] = 0;
                }
                let read_length = mux_stream.read(&mut buf).await.unwrap();
                assert_eq!(read_length, length);
                for i in 0..cmp::min(length, to_write.len() - write_start) {
                    assert_eq!(buf[i], to_write[write_start + i]);
                }
            }
            let elapsed = start.elapsed();
            println!(
                "{:5} Done {:10} in {:2.2}",
                i,
                to_write.len(),
                elapsed.as_secs_f64()
            );
            mux_stream.write_all(&[BREAK]).await.unwrap();
            mux_stream.shutdown().await.unwrap();
        });
    }
    println!("==== spawned them all ====");

    for _ in 0..join_set.len() {
        join_set.join_next().await;
    }
    println!("Done");
}
