mod packet_utils;
mod packet_processors;
mod net;
mod states;

use std::{net::{TcpStream, ToSocketAddrs}, thread::park};
use std::io;
use futures::executor::ThreadPool;
use std::net::SocketAddr;
use rio::Rio;
use crate::packet_processors::PacketProcessor;
use std::sync::{Arc, Mutex};
use futures::Future;
use futures::task::{Context, Poll, Waker};
use std::pin::Pin;
use std::time::{Duration};
use std::{thread};
use crate::net::{BotInfo, process_packet};
use crate::states::login;

static mut WAITING: Option<Mutex<Vec<Sleep>>> = None;

fn main() -> io::Result<()> {
    let ring = rio::new()?;
    let addrs = "localhost:25566".to_socket_addrs().unwrap().nth(0).unwrap();
    let thread_pool = ThreadPool::new().unwrap();
    let packet_processor = Arc::new(PacketProcessor::new());

    thread::spawn(|| unsafe {
        loop {
            if WAITING.is_none() {
                WAITING = Some(Mutex::new(Vec::new()));
            }
            let mutex = WAITING.as_ref().unwrap();
            let mut guard = mutex.lock().unwrap();
            for s in guard.iter_mut() {
                s.waker.as_ref().unwrap().clone().wake();
                thread::sleep(Duration::from_millis(4));
            }
            guard.clear();
            drop(guard);
        }
    });

    for i in 0..500 {
        thread_pool.spawn_ok(spawn_bot(ring.clone(), thread_pool.clone(), addrs.clone(), packet_processor.clone(), format!("test{}", i).to_string()));
    }

    loop {
        park();
    }
}

pub async fn spawn_bot(ring: Rio, pool: ThreadPool, addrs: SocketAddr, packet_processor: Arc<PacketProcessor>, name: String) {
    let pool_temp = pool.clone();
    let bot_task = async move {
        let mut bot = BotInfo {
            ring,
            pool,
            channel: Arc::new(TcpStream::connect(addrs).unwrap()),
            compression_threshold: 0,
            state: 0,
            packet_processor,
        };
        //login sequence
        BotInfo::send_packet(bot.clone(), login::write_handshake_packet(754, "".to_string(), 0, 2)).await;
        BotInfo::send_packet(bot.clone(), login::write_login_start_packet(name)).await;
        loop {
            process_packet(&mut bot).await;
            Sleep::new().await;
        }
    };
    pool_temp.spawn_ok(bot_task);
}

#[derive(Clone)]
pub struct Sleep {
    done: bool,
    waker: Option<Waker>,
}

impl Sleep {
    pub fn new() -> Sleep {
        Sleep { done: false, waker: None }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.done {
            Poll::Ready(())
        } else {
            self.as_mut().waker = Some(cx.waker().clone());
            unsafe {
                if WAITING.is_none() {
                    WAITING = Some(Mutex::new(Vec::new()));
                }
                let mutex = WAITING.as_ref().unwrap();
                let mut guard = mutex.lock().unwrap();
                guard.push(self.clone());
                self.done = true;
            }
            Poll::Pending
        }
    }
}
