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

pub async fn read_needed(bot: &BotInfo, packet: &mut Buf, needed: usize) -> usize {
    packet.buffer.reserve(needed);
    let mut received = 0;
    let len = packet.buffer.len();
    unsafe { packet.buffer.set_len(len + needed); }

    while received < needed {
        unsafe { received += read_socket(bot, &packet.offset((len + received) as u32)).await; }
    }
    received
}

pub async fn process_packet(bot: &mut BotInfo) {
    let packet_processor = bot.packet_processor.clone();

    //allocate buffer
    let mut packet = Buf::with_length(512);

    //read new packets
    let mut received = read_socket(bot, &packet).await;
    if received == 0 {
        return;
    }
    unsafe { packet.buffer.set_len(received); }
    let mut next = 0;

    //process all of the Minecraft packets received
    loop {
        //handle packet that have an incomplete size field
        if packet.buffer.len() as u32 - next < 3 {
            let needed = 3 - (packet.buffer.len() as u32 - next) as usize;

            received += read_needed(&bot, &mut packet, needed).await;
        }

        //read packet size
        let size = packet.read_var_u32() as usize;
        next += size as u32 + Buf::get_var_u32_size(size as u32);

        //handle incomplete packet
        if received < size + packet.get_reader_index() as usize {
            let needed = size + packet.get_reader_index() as usize - received;

            received += read_needed(&bot, &mut packet, needed).await;
        }

        //decompress if needed and parse the packet
        if bot.compression_threshold > 0 {
            let real_length = packet.read_var_u32();

            //buffer is compressed
            if real_length != 0 {
                let mut output = Buf::with_capacity(real_length);
                {
                    //decompress
                    let mut decompressor = ZlibDecoder::new(&mut output.buffer);
                    decompressor.write_all(
                        &packet.buffer[packet.get_reader_index() as usize
                            ..
                            (packet.get_reader_index() as usize
                                + size - Buf::get_var_u32_size(real_length) as usize)]).unwrap();
                }

                packet_processor.process_decode(&mut output, bot).await;
            } else {
                packet_processor.process_decode(&mut packet, bot).await;
            }
        } else {
            packet_processor.process_decode(&mut packet, bot).await;
        }

        //prepare for next packet and exit condition
        packet.set_reader_index(next);
        if packet.get_reader_index() >= packet.buffer.len() as u32 {
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
}

impl BotInfo {
    pub fn send_packet_async(bot: &BotInfo, buf: Buf) {
        let send = BotInfo::send_packet(bot.clone(), buf);
        bot.pool.spawn(send);
    }

    pub async fn send_packet(bot: BotInfo, buf: Buf) {
        let mut packet = buf;
        if bot.compression_threshold > 0 {
            packet = packet_processors::PacketCompressor::process_write(packet, &bot);
        }
        packet = packet_processors::PacketFramer::process_write(packet);
        let mut written = 0;
        let len = packet.buffer.len();
        while written < len {
            unsafe { written += bot.ring.write_at(&*bot.channel, &packet.offset(written as u32), 0).await.unwrap(); }
        }
    }
}