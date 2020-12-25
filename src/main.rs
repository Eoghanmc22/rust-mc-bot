mod packet_utils;
mod packet_processors;
mod login;
mod play;

use std::{net::{TcpStream, ToSocketAddrs}, thread::park};
use std::io;
use futures::executor::ThreadPool;
use std::net::SocketAddr;
use rio::Rio;
use crate::packet_processors::PacketProcessor;
use crate::packet_utils::Buf;
use std::sync::Arc;
use futures_locks::RwLock;
use std::ops::Deref;

fn main() -> io::Result<()> {
    let ring = rio::new()?;
    let addrs = "localhost:25565".to_socket_addrs().unwrap().nth(0).unwrap();
    let thread_pool = ThreadPool::new().unwrap();
    let packet_processor = Arc::new(PacketProcessor::new());

    for i in 0..1000 {
        thread_pool.spawn_ok(spawn_bot(ring.clone(), thread_pool.clone(), addrs.clone(), packet_processor.clone(), format!("test{}", i).to_string()));
    }

    loop {
        park();
    }
}

pub async fn spawn_bot(ring: Rio, pool: ThreadPool, addrs: SocketAddr, packet_processor: Arc<PacketProcessor>, name: String) {
    let pool_temp = pool.clone();
    let bot = Arc::new(RwLock::new(BotInfo {
        ring,
        pool,
        channel: TcpStream::connect(addrs).unwrap(),
        compression_threshold: 0,
        state: 0,
        packet_processor,
    }));
    let bot_1 = bot.clone();
    let bot_task = async move {
        //login sequence
        let read = Arc::new(bot_1.read().await);
        BotInfo::send_packet(read.clone(), login::write_handshake_packet(754, "".to_string(), 0, 2)).await;
        BotInfo::send_packet(read.clone(), login::write_login_start_packet(name)).await;
        drop(read);
        loop {
            process_packet(bot_1.clone(), false).await;
        }
    };
    pool_temp.spawn_ok(bot_task);
}

pub async fn process_packet(bot: Arc<RwLock<BotInfo>>, run_async: bool) {
    let bot_read = Arc::new(bot.read().await);
    let pool = bot_read.pool.clone();
    let packet_processor = bot_read.packet_processor.clone();
    let process = async move {
        let mut packet = Buf::with_length(512);
        let received = bot_read.ring.read_at(&bot_read.channel, &packet.buffer, 0).await.unwrap();
        unsafe { packet.buffer.set_len(received); }
        if received == 0 {
            return;
        }
        drop(bot_read);
        loop {
            let bot_read = Arc::new(bot.read().await);
            let (extra, more, size) = packet_processors::PacketFramer::process_read(&mut packet);
            let reader_index = packet.get_reader_index();
            if more > 0 {
                let extra_data = Buf::with_length(more);
                let mut total_received = 0;
                loop {
                    let received = bot_read.ring.read_at(&bot_read.channel, &extra_data.buffer, 0).await.unwrap() as u32;
                    total_received += received;
                    if total_received == more {
                        break;
                    }
                }
                packet.set_writer_index(packet.get_reader_index() + (size - more));
                packet.append(extra_data);
            }
            if bot_read.compression_threshold != 0 {
                if let Some(mut packet) = packet_processors::PacketCompressor::process_read(&mut packet, size) {
                    drop(bot_read);
                    packet_processor.process_decode(&mut packet, bot.clone()).await;
                } else {
                    drop(bot_read);
                    packet_processor.process_decode(&mut packet, bot.clone()).await;
                }
            } else {
                drop(bot_read);
                packet_processor.process_decode(&mut packet, bot.clone()).await;
            }
            if !extra {
                break;
            }
            packet.set_reader_index(reader_index + size);
        }
    };
    if run_async {
        pool.spawn_ok(process);
    } else {
        process.await;
    }
}

pub struct BotInfo {
    pub ring: Rio,
    pub pool: ThreadPool,
    pub channel: TcpStream,
    pub compression_threshold: i32,
    pub state: u8,
    pub packet_processor: Arc<PacketProcessor>,
}

impl BotInfo {
    pub fn send_packet_async<D: 'static + Deref<Target=BotInfo> + Send + Sync>(bot: Arc<D>, buf: Buf) {
        let pool = bot.pool.clone();
        let send = async move {
            let mut packet = buf;
            if bot.compression_threshold != 0 {
                packet = packet_processors::PacketCompressor::process_write(packet, bot.clone());
            }
            packet = packet_processors::PacketFramer::process_write(packet);
            //let mut written = 0;
            //let len = packet.buffer.len();
            //while written < len {
                /*written += */bot.ring.write_at(&bot.channel, &packet.buffer, 0).await.unwrap();
            //}
            drop(bot);
        };
        pool.spawn_ok(send);
    }

    pub async fn send_packet<D: 'static + Deref<Target=BotInfo> + Send + Sync>(bot: Arc<D>, buf: Buf) {
        let mut packet = buf;
        if bot.compression_threshold > 0 {
            packet = packet_processors::PacketCompressor::process_write(packet, bot.clone());
        }
        packet = packet_processors::PacketFramer::process_write(packet);
        //let mut written = 0;
        //let len = packet.buffer.len();
        //while written < len {
        /*written += */bot.ring.write_at(&bot.channel, &packet.buffer, 0).await.unwrap();
        //}
        drop(bot);
    }
}
