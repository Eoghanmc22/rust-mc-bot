use crate::packet_utils::Buf;
use libdeflater::Compressor;
use crate::states::{login, status, play};
use crate::{Bot, Compression, Error};

pub type Packet = fn(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression);

pub struct PacketFramer {}

pub struct PacketCompressor {}

pub fn lookup_packet(state: u8, packet: u8) -> Option<Packet> {
    match state {
        // login
        0 => {
            match packet {
                0x02 => return Some(login::process_login_success_packet),
                0x03 => return Some(login::process_set_compression_packet),
                _ => {}
            }
        }

        // status
        1 => {
            match packet {
                0x00 => return Some(status::process_status_response),
                0x01 => return Some(status::process_pong),
                _ => {}
            }
        }

        // play
        2 => {
            match packet {
                0x20 => return Some(play::process_keep_alive_packet), // KEEP_ALIVE
                0x25 => return Some(play::process_join_game), // JOIN_GAME
                0x19 => return Some(play::process_kick), // DISCONNECT
                0x39 => return Some(play::process_teleport), // PLAYER_POSITION_AND_LOOK
                _ => {}
            }
        }

        _ => println!("unknown state `{}`", state)
    }
    None
}

pub fn process_decode(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) -> Option<()> {
    let packet_id = buffer.read_var_u32().0 as u8;
    (lookup_packet(bot.state, packet_id)?)(buffer, bot, compression);
    Some(())
}

impl PacketFramer {
    pub fn process_write(buffer: Buf) -> Buf {
        let size = buffer.get_writer_index();
        let header_size = Buf::get_var_u32_size(size as u32);
        if header_size > 3 {
            panic!("header_size > 3")
        }
        let mut target = Buf::with_length(size as u32 + header_size);
        target.write_var_u32(size as u32);
        target.append(&buffer, buffer.get_writer_index() as usize);
        target
    }
}

impl PacketCompressor {
    pub fn process_write(mut buffer: Buf, bot: &Bot, compression: &mut Compression) -> Result<Buf, Error> {
        if buffer.get_writer_index() as i32 > bot.compression_threshold {
            let mut buf = Buf::new();
            compress_packet(&mut buffer, &mut compression.compressor, &mut buf)?;
            Ok(buf)
        } else {
            let mut buf = Buf::new();
            buf.write_var_u32(0);
            buf.append(&buffer, buffer.get_writer_index() as usize);
            Ok(buf)
        }
    }
}

pub fn compress_packet(packet: &Buf, compressor: &mut Compressor, compression_buffer: &mut Buf) -> Result<(), Error> {
    compression_buffer.write_var_u32(packet.get_writer_index());
    compression_buffer.ensure_writable(compressor.zlib_compress_bound(packet.get_writer_index() as usize) as u32);

    //compress
    let written =  compressor.zlib_compress(&packet.buffer, &mut compression_buffer.buffer)?;
    compression_buffer.set_writer_index(written as u32);

    Ok(())
}