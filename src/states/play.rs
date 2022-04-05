use crate::packet_utils::Buf;
use crate::{Bot, Compression};

pub fn process_keep_alive_packet(buffer : &mut Buf, bot : &mut Bot, compression: &mut Compression) {
    bot.send_packet(write_keep_alive_packet(buffer.read_u64()), compression);
}

pub fn process_kick(buffer : &mut Buf, bot : &mut Bot, _compression: &mut Compression) {
    println!("bot was kicked for \"{}\"", buffer.read_sized_string());
    bot.kicked = true;
}

pub fn process_join_game(buffer : &mut Buf, bot : &mut Bot, compression: &mut Compression) {
    bot.entity_id = buffer.read_u32();
    bot.send_packet(crate::play::write_client_settings(), compression);
}

pub fn process_teleport(buffer : &mut Buf, bot : &mut Bot, compression: &mut Compression) {
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
    bot.send_packet(write_tele_confirm(buffer.read_var_u32().0), compression);
    bot.teleported = true;
}

pub fn write_chat_message(message: &str) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x03);

    buf.write_sized_str(message);

    buf
}

pub fn write_animation(off_hand: bool) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x2C);

    buf.write_var_u32(if off_hand { 1 } else { 0 });

    buf
}

pub fn write_entity_action(entity_id: u32, action_id: u32, jump_boost: u32) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x1B);

    buf.write_var_u32(entity_id);
    buf.write_var_u32(action_id);
    buf.write_var_u32(jump_boost);

    buf
}

pub fn write_held_slot(slot: u16) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x25);

    buf.write_u16(slot);

    buf
}

pub fn write_tele_confirm(id : u32) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x00);

    buf.write_var_u32(id);

    buf
}

pub fn write_keep_alive_packet(id : u64) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x0F);

    buf.write_u64(id);

    buf
}

pub fn write_current_pos(bot : &Bot) -> Buf {
    write_pos(bot.x, bot.y, bot.z, 0.0,0.0)
}

pub fn write_pos(x : f64, y : f64, z : f64, yaw : f32, pitch : f32) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x12);

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
    buf.write_bool(true);
    buf.write_bool(false);

    buf
}