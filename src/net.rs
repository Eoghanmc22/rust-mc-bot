use crate::packet_utils::Buf;
use flate2::write::ZlibDecoder;
use std::io::{Write, Read, ErrorKind};
use crate::{packet_processors, Bot};

pub fn read_socket(bot: &mut Bot<'_>, packet: &mut Buf) -> bool {
    if bot.kicked {
        return false;
    }
    let w_i = packet.get_writer_index();
    let result = bot.stream.read(&mut packet.buffer[w_i as usize..])/*.expect("unable to read socket") as u32*/;
    match result {
        Ok(written) => {
            packet.set_writer_index(packet.get_writer_index() + written as u32);
            true
        }
        Err(e) => {
            match e.kind() {
                ErrorKind::WouldBlock => {}
                _ => {
                    bot.kicked = true;
                    println!("unable to read socket: {:?}", e)
                }
            }
            false
        }
    }
}

pub fn buffer(temp_buf: &mut Buf, buffering_buf: &mut Buf) {
    buffering_buf.write_bytes(&temp_buf.buffer[temp_buf.get_reader_index() as usize..temp_buf.get_writer_index() as usize]);
}

pub fn unbuffer(temp_buf: &mut Buf, buffering_buf: &mut Buf) {
    if buffering_buf.get_writer_index() != 0 {
        temp_buf.write_bytes(&buffering_buf.buffer[..buffering_buf.get_writer_index() as usize]);
        buffering_buf.set_writer_index(0);
    }
}

pub fn process_packet(bot: &mut Bot<'_>, packet_buf: &mut Buf, mut decompression_buf: &mut Buf) {
    packet_buf.set_reader_index(0);
    packet_buf.set_writer_index(0);
    let packet_processor = bot.packet_processor.clone();

    // Read new packets
    unbuffer(packet_buf, &mut bot.buffering_buf);
    while read_socket(bot, packet_buf) {
        if packet_buf.get_writer_index() == 0 {
            bot.kicked = true;
            println!("No new data");
            return;
        }
        let len = packet_buf.buffer.len();
        if packet_buf.get_writer_index() == len as u32 {
            packet_buf.buffer.reserve(len);
            unsafe { packet_buf.buffer.set_len(len * 2); }
            read_socket(bot, packet_buf);
            if bot.kicked {
                return;
            }
        } else {
            break;
        }
    }
    if bot.kicked {
        return;
    }
    let mut next = 0;

    // Process all of the Minecraft packets received
    loop {
        // Handle packet that have an incomplete size field
        if packet_buf.get_writer_index() as u32 - next < 3 {
            buffer(packet_buf, &mut bot.buffering_buf);
            break;
        }

        // Read packet size
        let tuple = packet_buf.read_var_u32();
        let size = tuple.0 as usize;
        next += tuple.0 + tuple.1;

        // Skip packets of 0 length
        if size == 0 {
            continue;
        }

        // Handle incomplete packet
        if packet_buf.get_writer_index() < size as u32 + packet_buf.get_reader_index() {
            packet_buf.set_reader_index(packet_buf.get_reader_index()-tuple.1);
            buffer(packet_buf, &mut bot.buffering_buf);
            break;
        }

        // Decompress if needed and parse the packet
        if bot.compression_threshold > 0 {
            let real_length_tuple = packet_buf.read_var_u32();
            let real_length = real_length_tuple.0;

            // Buffer is compressed
            if real_length != 0 {
                decompression_buf.set_reader_index(0);
                decompression_buf.set_writer_index(0);
                unsafe {
                    if real_length as usize > decompression_buf.buffer.len() {
                        decompression_buf.buffer.reserve(real_length as usize - decompression_buf.buffer.len());
                        decompression_buf.buffer.set_len(decompression_buf.buffer.capacity());
                    }
                }

                {
                    let s = packet_buf.get_reader_index() as usize;
                    let e = packet_buf.get_reader_index() as usize + size -real_length_tuple.1 as usize;

                    if s > e {
                        println!("s {} > e {}, size: {}, tl: {}, ri: {}, wi: {}", s, e, size, real_length_tuple.1, packet_buf.get_reader_index(), packet_buf.get_writer_index());
                        bot.kicked = true;
                        break;
                    }

                    // Decompress
                    let mut decompressor = ZlibDecoder::new(&mut decompression_buf);
                    match decompressor.write_all(
                        &packet_buf.buffer[packet_buf.get_reader_index() as usize
                            ..
                            (packet_buf.get_reader_index() as usize
                                + size - real_length_tuple.1 as usize)]) {
                        Ok(x) => x,
                        Err(_) => {
                            println!("decompression error");
                            bot.kicked = true;
                            break;
                        },
                    };
                }

                packet_processor.process_decode(&mut decompression_buf, bot);
            } else {
                packet_processor.process_decode(packet_buf, bot);
            }
        } else {
            packet_processor.process_decode(packet_buf, bot);
        }
        if bot.kicked {
            break;
        }

        // Prepare for next packet and exit condition
        packet_buf.set_reader_index(next);
        if packet_buf.get_reader_index() >= packet_buf.get_writer_index() {
            break;
        }
    }
}

impl Bot<'_> {
    pub fn send_packet(&mut self, buf: Buf) {
        if self.kicked {
            return;
        }
        let mut packet = buf;
        if self.compression_threshold > 0 {
            packet = packet_processors::PacketCompressor::process_write(packet, &self);
        }
        packet = packet_processors::PacketFramer::process_write(packet);
        match self.stream.write_all(&packet.buffer[packet.get_reader_index() as usize..packet.get_writer_index() as usize]) {
            Ok(_) => {}
            Err(e) => {
                self.kicked = true;
                println!("could not write to buf: {}", e);
            }
        }
    }
}