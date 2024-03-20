use crate::packet_utils::Buf;
use crate::{Bot, Compression};

// Clientbound Keep Alive (play)
pub fn process_keep_alive_packet(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.send_packet(write_keep_alive_packet(buffer.read_u64()), compression);
}

// Disconnect (login/config/play)
pub fn process_kick(buffer: &mut Buf, bot: &mut Bot, _compression: &mut Compression) {
    println!("bot was kicked for \"{}\"", buffer.read_sized_string());
    bot.kicked = true;
}

// Login (play)
pub fn process_join_game(buffer: &mut Buf, bot: &mut Bot, _compression: &mut Compression) {
    bot.entity_id = buffer.read_u32();
}

// Synchronize Player Position
pub fn process_teleport(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
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

// Chat Message
pub fn write_chat_message(message: &str) -> Buf {
    // ClientChatMessagePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x05);

    buf.write_sized_str(message);

    // 1.19 signing fields
    buf.write_u64(0); // timestamp
    buf.write_u64(0); // salt
    buf.write_bool(false); // has signature
    buf.write_var_u32(0); // count
    buf.write_bytes(&[0; 3]); // bitset

    buf
}

// Swing Arm
pub fn write_animation(off_hand: bool) -> Buf {
    // ClientAnimationPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x33);

    buf.write_var_u32(if off_hand { 1 } else { 0 });

    buf
}

// Player Command
pub fn write_entity_action(entity_id: u32, action_id: u32, jump_boost: u32) -> Buf {
    // ClientEntityActionPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x22);

    buf.write_var_u32(entity_id);
    buf.write_var_u32(action_id);
    buf.write_var_u32(jump_boost);

    buf
}

// Set Held Item (serverbound)
pub fn write_held_slot(slot: u16) -> Buf {
    // ClientHeldItemChangePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x2C);

    buf.write_u16(slot);

    buf
}

// Confirm Teleportation
pub fn write_tele_confirm(id: u32) -> Buf {
    // ClientTeleportConfirmPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x00);

    buf.write_var_u32(id);

    buf
}

// Serverbound Keep Alive (play)
pub fn write_keep_alive_packet(id: u64) -> Buf {
    // ClientKeepAlivePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x15);

    buf.write_u64(id);

    buf
}

pub fn write_current_pos(bot: &Bot) -> Buf {
    write_pos(bot.x, bot.y, bot.z, 0.0, 0.0)
}

// Set Player Position and Rotation
pub fn write_pos(x: f64, y: f64, z: f64, yaw: f32, pitch: f32) -> Buf {
    // ClientPlayerPositionAndRotationPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x18);

    buf.write_f64(x);
    buf.write_f64(y);
    buf.write_f64(z);

    buf.write_f32(yaw);
    buf.write_f32(pitch);

    buf.write_bool(false);

    buf
}
