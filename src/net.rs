use rio::{AsIoVec, AsIoVecMut, Rio};
use crate::packet_utils::Buf;
use flate2::write::ZlibDecoder;
use std::io::Write;
use std::sync::Arc;
use std::net::TcpStream;
use crate::packet_processors::PacketProcessor;
use crate::packet_processors;
use rusty_pool::ThreadPool;

pub async fn read_socket<P>(bot: &BotInfo, packet: &P) -> usize
    where P: AsIoVec + AsIoVecMut
{
    bot.ring.read_at(&*bot.channel, packet, 0).await.unwrap()
}

pub async fn read_needed(bot: &BotInfo, packet: &mut Buf, needed: usize, offset : usize) -> usize {
    if offset + needed > packet.buffer.len() {
        let c_needed = packet.buffer.len() - offset + needed;
        packet.buffer.reserve(c_needed);
        let len = packet.buffer.len();
        unsafe { packet.buffer.set_len(len + c_needed); }
    }

    let mut received = 0;
    while received < needed {
        unsafe { received += read_socket(bot, &packet.offset(offset as u32)).await; }
    }
    received
}

pub async fn process_packet(bot: &mut BotInfo, packet : &mut Buf) {
    packet.set_reader_index(0);
    let packet_processor = bot.packet_processor.clone();

    //read new packets
    let mut received = read_socket(bot, packet).await;
    if received == 0 {
        return;
    }
    {
        let len = packet.buffer.len();
        if received == len {
            packet.buffer.reserve(len);
            unsafe { packet.buffer.set_len(len * 2); }
            //println!("new buf size: {}", packet.buffer.len())
        }
    }
    let mut next = 0;

    //process all of the Minecraft packets received
    loop {
        //handle packet that have an incomplete size field
        if received as u32 - next < 3 {
            let needed = 3 - (received as u32 - next) as usize;

            received += read_needed(&bot, packet, needed, received).await;
        }

        //read packet size
        let tuple = packet.read_var_u32();
        let size = tuple.0 as usize;
        next += tuple.0 + tuple.1;

        //handle incomplete packet
        if received < size + packet.get_reader_index() as usize {
            let needed = size + packet.get_reader_index() as usize - received;

            received += read_needed(&bot, packet, needed, received).await;
        }

        //decompress if needed and parse the packet
        if bot.compression_threshold > 0 {
            let real_length_tuple = packet.read_var_u32();
            let real_length = real_length_tuple.0;

            //buffer is compressed
            if real_length != 0 {
                let mut output = Buf::with_capacity(real_length);
                {
                    //decompress
                    let mut decompressor = ZlibDecoder::new(&mut output);
                    decompressor.write_all(
                        &packet.buffer[packet.get_reader_index() as usize
                            ..
                            (packet.get_reader_index() as usize
                                + size -real_length_tuple.1 as usize)]).unwrap();
                }

                packet_processor.process_decode(&mut output, bot).await;
            } else {
                packet_processor.process_decode(packet, bot).await;
            }
        } else {
            packet_processor.process_decode(packet, bot).await;
        }
        if bot.kicked {
            break;
        }

        //prepare for next packet and exit condition
        packet.set_reader_index(next);
        if packet.get_reader_index() >= received as u32 {
            break;
        }
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
    pub kicked : bool,
    pub teleported : bool,
    pub x : f64,
    pub y : f64,
    pub z : f64,
}

impl BotInfo {
    pub fn send_packet_async(bot: &BotInfo, buf: Buf) {
        let send = BotInfo::send_packet(bot.clone(), buf);
        bot.pool.spawn(send);
    }

    pub async fn send_packet(bot: BotInfo, buf: Buf) {
        if bot.kicked {
            return;
        }
        let mut packet = buf;
        if bot.compression_threshold > 0 {
            packet = packet_processors::PacketCompressor::process_write(packet, &bot);
        }
        packet = packet_processors::PacketFramer::process_write(packet);
        let mut written = 0;
        let len = packet.get_writer_index() as usize;
        while written < len {
            unsafe { written += bot.ring.write_at(&*bot.channel, &packet.offset(written as u32), 0).await.unwrap(); }
        }
    }
}