use crate::packet_utils::Buf;
use crate::{Bot, Compression};

/// Status Response
pub fn process_status_response(buffer: &mut Buf, _bot: &mut Bot, _compression: &mut Compression) {
    let server_response = buffer.read_sized_string();
    println!("got response {}", server_response)
}

/// Ping Response (status)
pub fn process_pong(buffer: &mut Buf, _bot: &mut Bot, _compression: &mut Compression) {
    let payload = buffer.read_sized_string();
    println!("got pong {}", payload)
}

/// Status Request
pub fn write_status_request() -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x00);

    buf
}

/// Ping Request (status)
pub fn write_ping(payload: u64) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x01);

    buf.write_u64(payload);

    buf
}
