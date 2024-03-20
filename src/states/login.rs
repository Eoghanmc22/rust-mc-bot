use crate::packet_utils::Buf;
use crate::{Bot, Compression, ProtocolState};

//c2s

/// Handshake
pub fn write_handshake_packet(
    protocol_version: u32,
    server_address: String,
    server_port: u16,
    next_state: u32,
) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x00);

    buf.write_var_u32(protocol_version);
    buf.write_sized_str(&server_address);
    buf.write_u16(server_port);
    buf.write_var_u32(next_state);

    buf
}

/// Login Start
pub fn write_login_start_packet(username: &str, uuid: u128) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x00);

    buf.write_sized_str(username);
    buf.write_u128(uuid);

    buf
}

/// Login Acknowledged
pub fn write_login_acknowledged() -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x03);

    buf
}

//s2c

/// Login Success
pub fn process_login_success_packet(
    buffer: &mut Buf,
    mut bot: &mut Bot,
    compression: &mut Compression,
) {
    let _uuid = buffer.read_u128();
    let _name = buffer.read_sized_string();
    let _properties = buffer.read_var_u32();

    bot.state = ProtocolState::Config;

    bot.send_packet(write_login_acknowledged(), compression)
}

/// Set Compression
pub fn process_set_compression_packet(
    buf: &mut Buf,
    mut bot: &mut Bot,
    _compression: &mut Compression,
) {
    bot.compression_threshold = buf.read_var_u32().0 as i32;
}
