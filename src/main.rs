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
        BotInfo::send_packet(&bot, login::write_handshake_packet(754, "".to_string(), 0, 2)).await;
        BotInfo::send_packet(&bot, login::write_login_start_packet(name)).await;
        loop {
            process_packet(&mut bot).await;
        }
    };
    pool_temp.spawn_ok(bot_task);
}

pub async fn process_packet(bot: &mut BotInfo) {
    let packet_processor = bot.packet_processor.clone();

        let mut packet = Buf::with_length(512);
        let received = bot.ring.read_at(&*bot.channel, &packet.buffer, 0).await.unwrap();
        unsafe { packet.buffer.set_len(received); }
        if received == 0 {
            return;
        }
        loop {
            let (extra, more, size) = packet_processors::PacketFramer::process_read(&mut packet);
            let reader_index = packet.get_reader_index();
            if more > 0 {
                let extra_data = Buf::with_length(more);
                let mut total_received = 0;
                loop {
                    let received = bot.ring.read_at(&*bot.channel, &extra_data.buffer, 0).await.unwrap() as u32;
                    total_received += received;
                    if total_received == more {
                        break;
                    }
                }
                packet.set_writer_index(packet.get_reader_index() + (size - more));
                packet.append(extra_data);
            }
            if bot.compression_threshold != 0 {
                if let Some(mut packet) = packet_processors::PacketCompressor::process_read(&mut packet, size) {
                    packet_processor.process_decode(&mut packet, bot).await;
                } else {
                    packet_processor.process_decode(&mut packet, bot).await;
                }
            } else {
                packet_processor.process_decode(&mut packet, bot).await;
            }
            if !extra {
                break;
            }
            packet.set_reader_index(reader_index + size);
        }
}

#[derive(Clone)]
pub struct BotInfo {
    pub ring: Rio,
    pub pool: ThreadPool,
    pub channel: Arc<TcpStream>,
    pub compression_threshold: i32,
    pub state: u8,
    pub packet_processor: Arc<PacketProcessor>,
}

impl BotInfo {
    pub fn send_packet_async(bot_in: &BotInfo, buf: Buf) {
        let bot = bot_in.clone();
        let send = async move {
            let mut packet = buf;
            if bot.compression_threshold != 0 {
                packet = packet_processors::PacketCompressor::process_write(packet, &bot);
            }
            packet = packet_processors::PacketFramer::process_write(packet);
            //let mut written = 0;
            //let len = packet.buffer.len();
            //while written < len {
                /*written += */bot.ring.write_at(&*bot.channel, &packet.buffer, 0).await.unwrap();
            //}
        };
        bot_in.pool.spawn_ok(send);
    }

    pub async fn send_packet(bot: &BotInfo, buf: Buf) {
        let mut packet = buf;
        if bot.compression_threshold > 0 {
            packet = packet_processors::PacketCompressor::process_write(packet, bot);
        }
        packet = packet_processors::PacketFramer::process_write(packet);
        //let mut written = 0;
        //let len = packet.buffer.len();
        //while written < len {
        /*written += */bot.ring.write_at(&*bot.channel, &packet.buffer, 0).await.unwrap();
        //}
    }
}
