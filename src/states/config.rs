use std::usize;

use crate::{packet_utils::Buf, Bot, Compression, ProtocolState};

pub fn process_cookie_request_packet(buf: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    let identifier = buf.read_sized_string();
    bot.send_packet(write_cookie_response(identifier), compression);
}

/// Finish Configuration
pub fn process_finish_configuration(
    _buffer: &mut Buf,
    bot: &mut Bot,
    compression: &mut Compression,
) {
    bot.send_packet(write_acknowledge_configuration(), compression);

    bot.state = ProtocolState::Play;
}

/// Clientbound Keep Alive (configuration)
pub fn process_keep_alive_packet(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.send_packet(write_keep_alive_packet(buffer.read_u64()), compression);
}

/// Ping (configuration)
pub fn process_ping(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.send_packet(write_pong(buffer.read_u32()), compression);
}

/// Add Resource Pack (configuration)
pub fn process_resource_pack(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.send_packet(
        write_acknowledge_resource_pack(buffer.read_u128()),
        compression,
    );
}

pub fn process_transfer(buffer: &mut Buf, bot: &mut Bot, _compression: &mut Compression) {
    // Goofy lifetimes
    let address = buffer.read_sized_string().to_owned();
    let port = buffer.read_u16();

    println!("Server requested transfer to {address}:{port} but it isnt implemented! Please turn off online mode! Disconnecting bot {}", bot.name);
    bot.kicked = true;
}

pub fn process_known_packs(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.send_packet(write_known_packets(buffer), compression);
}

pub fn write_cookie_response(identifier: &str) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x01);

    buf.write_sized_str(identifier);
    buf.write_bool(false);

    buf
}

/// Acknowledge Finish Configuration
pub fn write_acknowledge_configuration() -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x03);

    buf
}

/// Serverbound Keep Alive (configuration)
pub fn write_keep_alive_packet(id: u64) -> Buf {
    // ClientKeepAlivePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x04);

    buf.write_u64(id);

    buf
}

/// Pong (configuration)
pub fn write_pong(id: u32) -> Buf {
    // ClientKeepAlivePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x05);

    buf.write_u32(id);

    buf
}

/// Resource Pack Response (configuration)
pub fn write_acknowledge_resource_pack(id: u128) -> Buf {
    // ClientKeepAlivePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x06);

    buf.write_u128(id);
    buf.write_var_u32(3); // Accepted

    buf
}

pub fn write_known_packets(request: &mut Buf) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x07);

    buf.write_var_u32(0);

    buf
}

const VIEW_DISTANCE: u8 = 10u8;

/// Client Information (configuration)
pub fn write_client_settings() -> Buf {
    // ClientSettingsPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x00);

    buf.write_sized_str("en_US");
    buf.write_u8(VIEW_DISTANCE);
    buf.write_var_u32(0);
    buf.write_bool(true);
    buf.write_u8(0xFF);
    buf.write_var_u32(1);
    buf.write_bool(false);
    buf.write_bool(true);

    buf
}
