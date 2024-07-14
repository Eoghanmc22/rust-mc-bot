use libdeflater::Compressor;

use crate::packet_utils::Buf;
use crate::states::{config, login, play, status};
use crate::{Bot, Compression, Error, ProtocolState};

pub type PacketHandler = fn(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression);

pub struct PacketFramer {}

pub struct PacketCompressor {}

pub fn lookup_packet(state: ProtocolState, packet: u8) -> Option<PacketHandler> {
    match state {
        ProtocolState::Login => match packet {
            0x00 => return Some(play::process_kick),
            0x01 => return Some(login::process_encryption_request_packet),
            0x02 => return Some(login::process_login_success_packet),
            0x03 => return Some(login::process_set_compression_packet),
            0x04 => return Some(login::process_plugin_message_request),
            0x05 => return Some(login::process_cookie_request_packet),
            _ => {}
        },

        ProtocolState::Status => match packet {
            0x00 => return Some(status::process_status_response),
            0x01 => return Some(status::process_pong),
            _ => {}
        },

        ProtocolState::Config => match packet {
            0x00 => return Some(config::process_cookie_request_packet),
            0x02 => return Some(play::process_kick),
            0x03 => return Some(config::process_finish_configuration),
            0x04 => return Some(config::process_keep_alive_packet),
            0x05 => return Some(config::process_ping),
            0x09 => return Some(config::process_resource_pack),
            0x0B => return Some(config::process_transfer),
            0x0E => return Some(config::process_known_packs),
            _ => {}
        },

        ProtocolState::Play => {
            match packet {
                0x16 => return Some(play::process_cookie_request_packet), // KEEP_ALIVE
                0x26 => return Some(play::process_keep_alive_packet),     // KEEP_ALIVE
                0x2B => return Some(play::process_join_game),             // JOIN_GAME
                0x1D => return Some(play::process_kick),                  // DISCONNECT
                0x40 => return Some(play::process_teleport), // PLAYER_POSITION_AND_LOOK
                0x73 => return Some(config::process_transfer),
                _ => {}
            }
        }
    }
    None
}

pub fn process_decode(
    buffer: &mut Buf,
    bot: &mut Bot,
    compression: &mut Compression,
) -> Option<()> {
    let packet_id = buffer.read_var_u32().0 as u8;
    (lookup_packet(bot.state, packet_id)?)(buffer, bot, compression);
    Some(())
}

impl PacketFramer {
    pub fn process_write(buffer: Buf) -> Buf {
        let size = buffer.get_writer_index();
        let header_size = Buf::get_var_u32_size(size);
        if header_size > 3 {
            panic!("header_size > 3")
        }
        let mut target = Buf::with_length(size + header_size);
        target.write_var_u32(size);
        target.append(&buffer, buffer.get_writer_index() as usize);
        target
    }
}

impl PacketCompressor {
    pub fn process_write(
        buffer: Buf,
        bot: &Bot,
        compression: &mut Compression,
    ) -> Result<Buf, Error> {
        if buffer.get_writer_index() as i32 > bot.compression_threshold {
            let mut buf = Buf::new();
            compress_packet(&buffer, &mut compression.compressor, &mut buf)?;
            Ok(buf)
        } else {
            let mut buf = Buf::new();
            buf.write_var_u32(0);
            buf.append(&buffer, buffer.get_writer_index() as usize);
            Ok(buf)
        }
    }
}

pub fn compress_packet(
    packet: &Buf,
    compressor: &mut Compressor,
    compression_buffer: &mut Buf,
) -> Result<(), Error> {
    compression_buffer.write_var_u32(packet.get_writer_index());
    compression_buffer
        .ensure_writable(compressor.zlib_compress_bound(packet.get_writer_index() as usize) as u32);

    //compress
    let written = compressor.zlib_compress(&packet.buffer, &mut compression_buffer.buffer)?;
    compression_buffer.set_writer_index(written as u32);

    Ok(())
}
