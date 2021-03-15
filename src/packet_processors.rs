use crate::packet_utils::Buf;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;
use std::collections::HashMap;
use crate::BotInfo;
use crate::states::login;
use crate::states::play;

pub type Packet = fn(buffer: &mut Buf, bot: &mut BotInfo);

pub struct PacketFramer {}

pub struct PacketCompressor {}

pub struct PacketProcessor {
    packets: HashMap<u8, HashMap<u8, Packet>>
}

impl PacketProcessor {
    pub fn new() -> Self {
        let mut map = HashMap::with_capacity(4);

        //Define packets here
        let mut login: HashMap<u8, Packet> = HashMap::new();

        login.insert(0x02, login::process_login_success_packet);
        login.insert(0x03, login::process_set_compression_packet);

        map.insert(0, login);


        let mut play: HashMap<u8, Packet> = HashMap::new();

        play.insert(0x1F, play::process_keep_alive_packet);
        play.insert(0x24, play::process_join_game);
        play.insert(0x34, play::process_teleport);
        play.insert(0x19, play::process_kick);

        map.insert(1, play);

        PacketProcessor { packets: map }
    }
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
    pub fn process_write(buffer: Buf, bot: &BotInfo) -> Buf {
        if buffer.get_writer_index() as i32 > bot.compression_threshold {
            let mut buf = Buf::new();
            buf.write_var_u32(buffer.get_writer_index());
            let mut compressor = ZlibEncoder::new(&mut buf, Compression::fast());
            compressor.write_all(&buffer.buffer[0..buffer.get_writer_index() as usize]).unwrap();
            compressor.flush_finish().unwrap();
            buf
        } else {
            let mut buf = Buf::new();
            buf.write_var_u32(0);
            buf.append(&buffer, buffer.get_writer_index() as usize);
            buf
        }
    }
}

impl PacketProcessor {
    pub async fn process_decode(&self, buffer: &mut Buf, bot: &mut BotInfo) -> Option<()> {
        let packet_id = buffer.read_var_u32();
        (self.packets.get(&bot.state)?.get(&(packet_id as u8))?)(buffer, bot);
        Some(())
    }
}