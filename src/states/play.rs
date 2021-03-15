use crate::packet_utils::Buf;
use crate::BotInfo;

pub fn process_keep_alive_packet(buffer : &mut Buf, bot : &mut BotInfo) {
   BotInfo::send_packet_async(bot, write_keep_alive_packet(buffer.read_u64()));
}

pub fn process_kick(buffer : &mut Buf, bot : &mut BotInfo) {
    println!("bot was kicked for \"{}\"", buffer.read_sized_string());
    bot.kicked = true;
}

pub fn process_join_game(_buffer : &mut Buf, bot : &mut BotInfo) {
    BotInfo::send_packet_async(bot, crate::play::write_client_settings());
}

pub fn process_teleport(buffer : &mut Buf, bot : &mut BotInfo) {
    let x = buffer.read_f64();
    let y = buffer.read_f64();
    let z = buffer.read_f64();
    let _yaw = buffer.read_f32();
    let _pitch = buffer.read_f32();
    let flags = buffer.read_byte();
    if flags & 0b10000 == 0 {
        bot.x = x;
    } else {
        bot.x += x;
    }
    if flags & 0b01000 == 0 {
        bot.y = y;
    } else {
        bot.y += y;
    }
    if flags & 0b00100 == 0 {
        bot.z = z;
    } else {
        bot.z += z;
    }
    BotInfo::send_packet_async(bot, write_tele_confirm(buffer.read_var_u32()));
    bot.teleported = true;
}

pub fn write_tele_confirm(id : u32) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x00);

    buf.write_var_u32(id);
    buf
}

pub fn write_keep_alive_packet(id : u64) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x10);

    buf.write_u64(id);
    buf
}

pub fn write_current_pos(bot : &BotInfo) -> Buf {
    write_pos(bot.x, bot.y, bot.z, 0.0,0.0)
}

pub fn write_pos(x : f64, y : f64, z : f64, yaw : f32, pitch : f32) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x13);

    buf.write_f64(x);
    buf.write_f64(y);
    buf.write_f64(z);

    buf.write_f32(yaw);
    buf.write_f32(pitch);

    buf.write_bool(false);
    
    buf
}

const VIEW_DISTANCE: u8 = 10u8;

pub fn write_client_settings() -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x05);

    buf.write_sized_str("en_US");
    buf.write_u8(VIEW_DISTANCE);
    buf.write_var_u32(0);
    buf.write_bool(true);
    buf.write_u8(0xFF);
    buf.write_var_u32(0);

    buf
}