mod net;
mod packet_processors;
mod packet_utils;
mod states;

use crate::packet_utils::Buf;
use crate::states::login;
use libdeflater::{CompressionLvl, Compressor, Decompressor};
use mio::net::TcpStream;
use mio::{event, Events, Interest, Poll, Registry, Token};
use rand::prelude::*;
use states::play;
use std::collections::HashMap;
use std::io;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::{env, net::ToSocketAddrs};
use uuid::Uuid;

#[cfg(unix)]
use {mio::net::UnixStream, std::path::PathBuf};

// This rate limits the join rate of the bots
// Increasing it will cause the bots to join more quickly
const AVG_JOINS_PER_TICK: f64 = 5.0;

const SHOULD_MOVE: bool = true;
const MESSAGES: &[&str] = &["This is a chat message!", "Wow", "Server = on?"];

#[cfg(unix)]
const UDS_PREFIX: &str = "unix://";
const PROTOCOL_VERSION: u32 = 767;

type Error = Box<dyn std::error::Error + Send + Sync>;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        let name = args.get(0).unwrap();
        #[cfg(unix)]
        println!("usage: {} <ip:port or path> <count> [threads]", name);
        #[cfg(not(unix))]
        println!("usage: {} <ip:port> <count> [threads]", name);
        println!("example: {} localhost:25565 500", name);
        #[cfg(unix)]
        println!("example: {} unix:///path/to/socket 500", name);
        return Ok(());
    }

    let arg1 = args.get(1).unwrap();
    let arg2 = args.get(2).unwrap();
    let arg3 = args.get(3);

    let mut addrs = None;

    #[cfg(unix)]
    if let Some(unix_socket) = arg1.strip_prefix(UDS_PREFIX) {
        addrs = Some(Address::UNIX(PathBuf::from(unix_socket.to_owned())));
    }

    if addrs.is_none() {
        let mut parts = arg1.split(':');
        let ip = parts.next().expect("no ip provided");
        let port = parts
            .next()
            .map(|port_string| port_string.parse().expect("invalid port"))
            .unwrap_or(25565u16);

        let server = (ip, port)
            .to_socket_addrs()
            .expect("Not a socket address")
            .next()
            .expect("No socket address found");

        addrs = Some(Address::TCP(server));
    }

    // Cant be none because it would have panicked earlier
    let addrs = addrs.unwrap();

    let count: u32 = arg2
        .parse()
        .unwrap_or_else(|_| panic!("{} is not a number", arg2));
    let mut cpus = 1.max(num_cpus::get()) as u32;

    if let Some(str) = arg3 {
        cpus = str
            .parse()
            .unwrap_or_else(|_| panic!("{} is not a number", arg2));
    }

    println!("cpus: {}", cpus);

    let count_per_thread = count / cpus;
    let mut extra = count % cpus;
    let mut names_used = 0;

    if count > 0 {
        let mut threads = Vec::new();
        for _ in 0..cpus {
            let mut count = count_per_thread;

            if extra > 0 {
                extra -= 1;
                count += 1;
            }

            let addrs = addrs.clone();
            threads.push(std::thread::spawn(move || {
                start_bots(count, addrs, names_used, cpus)
            }));

            names_used += count;
        }

        for thread in threads {
            let _ = thread.join();
        }
    }
    Ok(())
}

pub struct Compression {
    compressor: Compressor,
    decompressor: Decompressor,
}

pub struct Bot {
    pub token: Token,
    pub stream: Stream,
    pub name: String,
    pub id: u32,
    pub entity_id: u32,
    pub compression_threshold: i32,
    pub state: ProtocolState,
    pub kicked: bool,
    pub teleported: bool,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub buffering_buf: Buf,
    pub joined: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ProtocolState {
    Status,
    Login,
    Config,
    Play,
}

pub fn start_bots(count: u32, addrs: Address, name_offset: u32, cpus: u32) {
    if count == 0 {
        return;
    }
    let mut poll = Poll::new().expect("could not unwrap poll");
    //todo check used cap
    let mut events = Events::with_capacity((count * 5) as usize);
    let mut map = HashMap::new();

    println!("{:?}", addrs);

    fn start_bot(bot: &mut Bot, compression: &mut Compression) {
        bot.joined = true;

        // socket ops
        bot.stream.set_ops();

        //login sequence
        let buf = login::write_handshake_packet(PROTOCOL_VERSION, "".to_string(), 0, 2);
        bot.send_packet(buf, compression);

        let uuid: u128 = Uuid::new_v4().as_u128();
        let buf = login::write_login_start_packet(&bot.name, uuid);
        bot.send_packet(buf, compression);

        println!("bot \"{}\" joined", bot.name);
    }

    let bots_per_tick = AVG_JOINS_PER_TICK / cpus as f64;
    let mut bots_this_tick = 0.0;
    let mut bots_joined = 0;

    let mut packet_buf = Buf::with_length(2000);
    let mut uncompressed_buf = Buf::with_length(2000);

    let mut compression = Compression {
        compressor: Compressor::new(CompressionLvl::default()),
        decompressor: Decompressor::new(),
    };

    let dur = Duration::from_millis(50);

    let mut tick_counter = 0;
    let action_tick = 4;

    'main: loop {
        let ins = Instant::now();

        if bots_joined < count {
            bots_this_tick += bots_per_tick;

            let registry = poll.registry();
            for bot in bots_joined..(bots_this_tick as u32 + bots_joined).min(count) {
                let token = Token(bot as usize);
                let name = "Bot_".to_owned() + &(name_offset + bot).to_string();

                let mut bot = Bot {
                    token,
                    stream: addrs.connect(),
                    name,
                    id: bot,
                    entity_id: 0,
                    compression_threshold: 0,
                    state: ProtocolState::Login,
                    kicked: false,
                    teleported: false,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    buffering_buf: Buf::with_length(200),
                    joined: false,
                };
                registry
                    .register(
                        &mut bot.stream,
                        bot.token,
                        Interest::READABLE | Interest::WRITABLE,
                    )
                    .expect("could not register");

                println!("spawn bot \"{}\" {}/{}", bot.name, bots_joined, count);

                map.insert(token, bot);

                bots_joined += 1;
                bots_this_tick -= 1.0;
            }
        }

        poll.poll(&mut events, Some(dur)).expect("couldn't poll");
        for event in events.iter() {
            if let Some(bot) = map.get_mut(&event.token()) {
                if event.is_writable() && !bot.joined {
                    start_bot(bot, &mut compression);
                }
                if event.is_readable() && bot.joined {
                    net::process_packet(
                        bot,
                        &mut packet_buf,
                        &mut uncompressed_buf,
                        &mut compression,
                    );
                    if bot.kicked {
                        println!("{} disconnected", bot.name);
                        let token = bot.token;
                        map.remove(&token).expect("kicked bot doesn't exist");

                        if map.is_empty() {
                            break 'main;
                        }
                    }
                }
            }
        }

        let elapsed = ins.elapsed();
        if elapsed < dur {
            std::thread::sleep(dur - elapsed);
        }

        let mut to_remove = Vec::new();

        for bot in map.values_mut() {
            if SHOULD_MOVE && bot.teleported {
                bot.x += rand::random::<f64>() * 1.0 - 0.5;
                bot.z += rand::random::<f64>() * 1.0 - 0.5;
                bot.send_packet(play::write_current_pos(bot), &mut compression);

                if (tick_counter + bot.id) % action_tick == 0 {
                    match rand::thread_rng().gen_range(0..=4u8) {
                        0 => {
                            // Send chat
                            bot.send_packet(
                                play::write_chat_message(
                                    MESSAGES.choose(&mut rand::thread_rng()).unwrap(),
                                ),
                                &mut compression,
                            );
                        }
                        1 => {
                            // Punch animation
                            bot.send_packet(
                                play::write_animation(rand::random()),
                                &mut compression,
                            );
                        }
                        2 => {
                            // Sneak
                            bot.send_packet(
                                play::write_entity_action(
                                    bot.entity_id,
                                    if rand::random() { 1 } else { 0 },
                                    0,
                                ),
                                &mut compression,
                            );
                        }
                        3 => {
                            // Sprint
                            bot.send_packet(
                                play::write_entity_action(
                                    bot.entity_id,
                                    if rand::random() { 3 } else { 4 },
                                    0,
                                ),
                                &mut compression,
                            );
                        }
                        4 => {
                            // Held item
                            bot.send_packet(
                                play::write_held_slot(rand::thread_rng().gen_range(0..9)),
                                &mut compression,
                            );
                        }
                        _ => {}
                    }
                }
            }

            if bot.kicked {
                to_remove.push(bot.token);
            }
        }

        for bot in to_remove {
            let _ = map.remove(&bot);
        }

        tick_counter += 1;
    }
}

#[derive(Clone, Debug)]
pub enum Address {
    #[cfg(unix)]
    UNIX(PathBuf),
    TCP(SocketAddr),
}

impl Address {
    pub fn connect(&self) -> Stream {
        match self {
            #[cfg(unix)]
            Address::UNIX(path) => {
                Stream::UNIX(UnixStream::connect(path).expect("Could not connect to the server"))
            }
            Address::TCP(address) => Stream::TCP(
                TcpStream::connect(address.to_owned()).expect("Could not connect to the server"),
            ),
        }
    }
}

pub enum Stream {
    #[cfg(unix)]
    UNIX(UnixStream),
    TCP(TcpStream),
}

impl Stream {
    pub fn set_ops(&mut self) {
        match self {
            Stream::TCP(s) => {
                s.set_nodelay(true).unwrap();
            }
            _ => {}
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => s.read(buf),
            Stream::TCP(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => s.write(buf),
            Stream::TCP(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => s.flush(),
            Stream::TCP(s) => s.flush(),
        }
    }
}

impl event::Source for Stream {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => s.register(registry, token, interests),
            Stream::TCP(s) => s.register(registry, token, interests),
        }
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => s.reregister(registry, token, interests),
            Stream::TCP(s) => s.reregister(registry, token, interests),
        }
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => s.deregister(registry),
            Stream::TCP(s) => s.deregister(registry),
        }
    }
}
