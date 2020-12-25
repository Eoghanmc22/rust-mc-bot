use crate::packet_utils::Buf;
use crate::BotInfo;

pub fn process_keep_alive_packet(buffer : &mut Buf, bot : &mut BotInfo) {
   BotInfo::send_packet_async(bot, write_keep_alive_packet(buffer.read_u64()));
}

pub fn write_keep_alive_packet(id : u64) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x10);

    buf.write_u64(id);
    buf
}