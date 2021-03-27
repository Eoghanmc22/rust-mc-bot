use crate::packet_utils::Buf;
use flate2::write::ZlibDecoder;
use std::io::{Write, Read};
use crate::{packet_processors, Bot};

pub fn read_socket(bot: &mut Bot<'_>, packet: &mut Buf) {
    let w_i = packet.get_writer_index();
    let written = bot.stream.read(&mut packet.buffer[w_i as usize..]).expect("unable to read socket") as u32;
    packet.set_writer_index(packet.get_writer_index() + written);
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

    //read new packets
    unbuffer(packet_buf, &mut bot.buffering_buf);
    read_socket(bot, packet_buf);
    if packet_buf.get_writer_index() == 0 {
        return;
    }
    loop {
        let len = packet_buf.buffer.len();
        if packet_buf.get_writer_index() == len as u32 {
            packet_buf.buffer.reserve(len);
            unsafe { packet_buf.buffer.set_len(len * 2); }
            read_socket(bot, packet_buf);
        } else {
            break;
        }
    }
    let mut next = 0;

    //process all of the Minecraft packets received
    loop {
        //handle packet that have an incomplete size field
        if packet_buf.get_writer_index() as u32 - next < 3 {
            buffer(packet_buf, &mut bot.buffering_buf);
            break;
        }

        //read packet size
        let tuple = packet_buf.read_var_u32();
        let size = tuple.0 as usize;
        next += tuple.0 + tuple.1;

        //handle incomplete packet
        if packet_buf.get_writer_index() < size as u32 + packet_buf.get_reader_index() {
            packet_buf.set_reader_index(packet_buf.get_reader_index()-tuple.1);
            buffer(packet_buf, &mut bot.buffering_buf);
            break;
        }

        //decompress if needed and parse the packet
        if bot.compression_threshold > 0 {
            let real_length_tuple = packet_buf.read_var_u32();
            let real_length = real_length_tuple.0;

            //buffer is compressed
            if real_length != 0 {
                decompression_buf.set_reader_index(0);
                decompression_buf.set_writer_index(0);
                unsafe {
                    decompression_buf.buffer.set_len(0);
                }
                decompression_buf.buffer.reserve(real_length as usize);

                {
                    //decompress
                    let mut decompressor = ZlibDecoder::new(&mut decompression_buf);
                    decompressor.write_all(
                        &packet_buf.buffer[packet_buf.get_reader_index() as usize
                            ..
                            (packet_buf.get_reader_index() as usize
                                + size -real_length_tuple.1 as usize)]).unwrap();
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

        //prepare for next packet and exit condition
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
        self.stream.write_all(&packet.buffer[packet.get_reader_index() as usize..packet.get_writer_index() as usize]).expect("could not write buffer");
    }
}