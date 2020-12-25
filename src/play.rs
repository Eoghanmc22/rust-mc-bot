use crate::packet_utils::Buf;
use futures_locks::RwLockWriteGuard;
use crate::BotInfo;
use std::sync::Arc;

pub fn process_keep_alive_packet(buffer : &mut Buf, bot : RwLockWriteGuard<BotInfo>) {
   BotInfo::send_packet_async(Arc::new(bot), write_keep_alive_packet(buffer.read_u64()));
}

pub fn write_keep_alive_packet(id : u64) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x10);

    buf.write_u64(id);
    buf
}