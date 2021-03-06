mod packet_utils;
mod packet_processors;
mod net;
mod states;

use std::{net::ToSocketAddrs, env};
use std::io;
use mio::{Poll, Events, Token, Interest};
use std::net::SocketAddr;
use states::play;
use std::collections::HashMap;
use mio::net::TcpStream;
use crate::packet_processors::PacketProcessor;
use crate::states::login;
use crate::packet_utils::Buf;
use std::time::{Duration, Instant};

const SHOULD_MOVE: bool = true;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        let name = args.get(0).unwrap();
        println!("usage: {} <ip:port> <count> [threads]", name);
        println!("example: {} localhost:25565 500", name);
        return Ok(());
    }

    let arg1 = args.get(1).unwrap();
    let arg2 = args.get(2).unwrap();
    let arg3 = args.get(3);

    let addrs = arg1.to_socket_addrs().expect(&format!("{} is not a ip", arg1)).nth(0).expect(&format!("{} is not a ip", arg1));
    let count: u32 = arg2.parse().expect(&format!("{} is not a number", arg2));
    let mut cpus = 1.max(num_cpus::get()) as u32;

    if let Option::Some(str) = arg3 {
        cpus = str.parse().expect(&format!("{} is not a number", arg2));
    }

    println!("cpus: {}", cpus);

    let count_per_thread = count/cpus;
    let mut extra = 0;
    if count_per_thread == 0 && count != 0 {
        extra = count;
    } else if count % cpus != 0 {
        extra = count % cpus;
    }

    if count_per_thread == 0 && extra > 0 {
        start_bots(extra, addrs.clone(), 0, cpus);
        return Ok(());
    } else if count_per_thread > 0 {
        let mut threads = Vec::new();
        for cpu in 0..cpus {
            let addrs = addrs.clone();
            threads.push(std::thread::spawn(move || { start_bots(count_per_thread, addrs, cpu, cpus) }))
        }

        start_bots(extra, addrs.clone(), cpus, cpus);

        for thread in threads {
            let _ = thread.join();
        }
    }
    Ok(())
}

pub struct Bot<'a> {
    pub token : Token,
    pub stream : TcpStream,
    pub name : String,
    pub packet_processor: &'a PacketProcessor,
    pub compression_threshold: i32,
    pub state: u8,
    pub kicked : bool,
    pub teleported : bool,
    pub x : f64,
    pub y : f64,
    pub z : f64,
    pub buffering_buf : Buf,
    pub joined : bool
}

pub fn start_bots(count : u32, addrs : SocketAddr, bunch : u32, cpus: u32) {
    if count == 0 {
        return;
    }
    let mut poll = Poll::new().expect("could not unwrap poll");
    //todo check used cap
    let mut events = Events::with_capacity((count * 5) as usize);
    let mut map = HashMap::new();
    let packet_handler = PacketProcessor::new();

    fn start_bot(bot : &mut Bot) {
        bot.joined = true;
        //login sequence
        let buf = login::write_handshake_packet(755, "".to_string(), 0, 2);
        bot.send_packet(buf);

        let buf = login::write_login_start_packet(&bot.name);
        bot.send_packet(buf);

        println!("bot \"{}\" joined", bot.name);
    }

    let bots_per_tick = (20.0/cpus as f64).round() as u32;
    let mut bots_joined = 0;

    let mut packet_buf = Buf::with_length(2000);
    let mut uncompressed_buf = Buf::with_length(2000);
    let dur = Duration::from_millis(50);

    loop {
        if bots_joined < count {
            let registry = poll.registry();
            for bot in bots_joined..(bots_per_tick + bots_joined) {
                let token = Token(bot as usize);
                let mut name = String::new();
                name.push_str("Bot_");
                name.push_str((count * bunch + bot).to_string().as_str());

                let mut bot = Bot { token, stream : TcpStream::connect(addrs).expect("Could not connect to the server"), name, packet_processor: &packet_handler, compression_threshold: 0, state: 0, kicked: false, teleported: false, x: 0.0, y: 0.0, z: 0.0, buffering_buf: Buf::with_length(200), joined : false };
                registry.register(&mut bot.stream, bot.token, Interest::READABLE.add(Interest::WRITABLE)).expect("could not register");

                map.insert(token, bot);
            }
            bots_joined += bots_per_tick;
        }

        let ins = Instant::now();
        poll.poll(&mut events, Some(dur)).expect("couldn't poll");
        for event in events.iter() {
            if let Some(bot) = map.get_mut(&event.token()) {
                if event.is_writable() && !bot.joined {
                    start_bot(bot);
                }
                if event.is_readable() && bot.joined {
                    net::process_packet(bot, &mut packet_buf, &mut uncompressed_buf);
                    if bot.kicked {
                        let token = bot.token;
                        map.remove(&token).expect("kicked bot doesn't exist");
                    }
                }
            }
        }

        let elapsed = ins.elapsed();
        if elapsed < dur {
            std::thread::sleep(dur-elapsed);
        }

        let mut to_remove = Vec::new();

        for bot in map.values_mut() {
            if SHOULD_MOVE && bot.teleported {
                bot.x += rand::random::<f64>()*1.0-0.5;
                bot.z += rand::random::<f64>()*1.0-0.5;
                bot.send_packet(play::write_current_pos(bot));
            }
            if bot.kicked {
                to_remove.push(bot.token);
            }
        }

        for bot in to_remove {
            let _ = map.remove(&bot);
        }
    }
}
