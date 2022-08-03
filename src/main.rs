mod packet_utils;
mod packet_processors;
mod net;
mod states;

use std::net::ToSocketAddrs;
use std::{io, thread};
use mio::{Poll, Events, Token, Interest, event, Registry};
use std::net::SocketAddr;
use states::play;
use std::collections::HashMap;
use mio::net::TcpStream;
use crate::states::login;
use crate::packet_utils::Buf;
use std::time::{Duration, Instant};
use std::io::{Read, Write};
use std::str::FromStr;
use anyhow::{bail, Context};
use libdeflater::{CompressionLvl, Compressor, Decompressor};
use rand::prelude::*;
use clap::Parser;
use env_logger::Env;
use human_panic::setup_panic;
use log::info;

#[cfg(unix)]
use {mio::net::UnixStream, std::path::PathBuf};

const SHOULD_MOVE: bool = true;

const PROTOCOL_VERSION: u32 = 758;

#[cfg(unix)]
const UDS_PREFIX: &str = "unix://";

const MESSAGES: &[&str] = &["This is a chat message!", "Wow", "Server = on?"];

fn main() -> anyhow::Result<()> {
    setup_panic!();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let Args { server, count, threads } = Args::parse();

    let threads = if threads == 0 {
        thread::available_parallelism().context("Could not retrieve parallelism. Try specifying a thread count with -p THREADS")?.get() as u32
    } else {
        threads
    };

    info!("Threads: {threads}");

    let count_per_thread = count / threads;
    let extra = count % threads;
    let mut names_used = 0;

    if count > 0 {
        let mut handles = Vec::new();
        for thread_id in 0..threads {
            let count = count_per_thread + if thread_id < extra { 1 } else { 0 };

            let addrs = server.clone();
            handles.push(thread::spawn(move || { start_bots(count, addrs, names_used, threads) }));

            names_used += count;
        }

        for thread in handles {
            let _ = thread.join();
        }
    }
    Ok(())
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(help = "The ip address of the server to connect to")]
    server: Address,
    #[clap(help = "The amount of bots to spawn")]
    count: u32,
    #[clap(short = 'p', default_value = "0", help = "The number of threads to create (0 for auto)")]
    threads: u32
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
    pub state: u8,
    pub kicked: bool,
    pub teleported: bool,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub buffering_buf: Buf,
    pub joined: bool
}

pub fn start_bots(count : u32, addrs : Address, name_offset : u32, cpus: u32) {
    if count == 0 {
        return;
    }

    let mut poll = Poll::new().expect("Create poll");
    let mut events = Events::with_capacity(count as usize * 2);
    let mut bots = HashMap::new();

    // todo improve rate handling
    let bots_per_tick = (1.0/cpus as f64).ceil() as u32;
    let mut bots_joined = 0;

    let mut packet_buf = Buf::with_length(2000);
    let mut uncompressed_buf = Buf::with_length(2000);
    let mut compression = Compression { compressor: Compressor::new(CompressionLvl::default()), decompressor: Decompressor::new() };

    let start_time = Instant::now();
    let tick_duration = Duration::from_millis(50);

    let mut tick_counter = 0;
    let action_tick = 4;

    loop {
        let tick_start = Instant::now();
        let tick_end = start_time + tick_duration * tick_counter;
        let max_time = tick_end - tick_start;

        // Connect additional bots
        if bots_joined < count {
            let registry = poll.registry();
            for bot in bots_joined..(bots_per_tick + bots_joined).min(count) {
                let token = Token(bot as usize);
                let name = format!("Bot_{}", name_offset + bot);

                let mut bot = Bot { token, stream : addrs.connect(), name, id: bot, entity_id: 0, compression_threshold: 0, state: 0, kicked: false, teleported: false, x: 0.0, y: 0.0, z: 0.0, buffering_buf: Buf::with_length(200), joined : false };
                registry.register(&mut bot.stream, bot.token, Interest::READABLE | Interest::WRITABLE).expect("Register");
                bots.insert(token, bot);

                bots_joined += 1;
            }
        }

        // Poll events
        poll.poll(&mut events, Some(Duration::ZERO)).expect("Poll events");
        for event in &events {
            if let Some(bot) = bots.get_mut(&event.token()) {
                // Set up bot if needed
                if event.is_writable() && !bot.joined && !bot.kicked {
                    // Set socket ops
                    bot.stream.set_ops();

                    // Send login sequence
                    let buf = login::write_handshake_packet(PROTOCOL_VERSION, "", 0, 2);
                    bot.send_packet(buf, &mut compression);

                    let buf = login::write_login_start_packet(&bot.name);
                    bot.send_packet(buf, &mut compression);

                    info!("`{bot}` joined", bot = bot.name);
                    bot.joined = true;
                }

                // Read new data
                if event.is_readable() && bot.joined && !bot.kicked {
                    net::process_packet(bot, &mut packet_buf, &mut uncompressed_buf, &mut compression);
                }
            }
        }

        // Sleep
        let elapsed = tick_start.elapsed();
        if elapsed < max_time {
            thread::sleep(tick_duration - elapsed);
        }

        // Tick bots
        for bot in bots.values_mut() {
            if SHOULD_MOVE && bot.teleported && !bot.kicked {
                bot.x += random::<f64>() * 1.0 - 0.5;
                bot.z += random::<f64>() * 1.0 - 0.5;
                bot.send_packet(play::write_current_pos(bot), &mut compression);

                if (tick_counter + bot.id) % action_tick == 0 {
                    match thread_rng().gen_range(0..=4u8) {
                        0 => {
                            // Send chat
                            bot.send_packet(play::write_chat_message(MESSAGES.choose(&mut thread_rng()).unwrap()), &mut compression);
                        }
                        1 => {
                            // Punch animation
                            bot.send_packet(play::write_animation(random()), &mut compression);
                        }
                        2 => {
                            // Sneak
                            bot.send_packet(play::write_entity_action(bot.entity_id, if random() { 1 } else { 0 }, 0), &mut compression);
                        }
                        3 => {
                            // Sprint
                            bot.send_packet(play::write_entity_action(bot.entity_id, if random() { 3 } else { 4 }, 0), &mut compression);
                        }
                        4 => {
                            // Held item
                            bot.send_packet(play::write_held_slot(thread_rng().gen_range(0..9)), &mut compression);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Remove kicked bots
        bots.retain(|_, bot| {
            if bot.kicked {
                info!("`{bot}` disconnected", bot = bot.name);
                poll.registry().deregister(&mut bot.stream).expect("Deregister");

                false
            } else {
                true
            }
        });

        // Kill thread if all related bots have been kicked
        if bots.is_empty() {
            break;
        }

        tick_counter += 1;
    }
}

#[derive(Clone, Debug)]
pub enum Address {
    #[cfg(unix)]
    UNIX(PathBuf),
    TCP(SocketAddr)
}

impl Address {
    pub fn connect(&self) -> Stream {
        match self {
            #[cfg(unix)]
            Address::UNIX(path) => {
                Stream::UNIX(UnixStream::connect(path).expect("Could not connect to the server"))
            }
            Address::TCP(address) => {
                Stream::TCP(TcpStream::connect(address.to_owned()).expect("Could not connect to the server"))
            }
        }
    }
}

impl FromStr for Address {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(uds) = s.strip_prefix(UDS_PREFIX) {
            if cfg!(unix) {
                Ok(Address::UNIX(PathBuf::from(uds.to_owned())))
            } else {
                bail!("Unix domain sockets are not supported on this platform")
            }
        } else {
            Ok(Address::TCP(
                s.to_socket_addrs().context("Could not parse address")?
                    .next().context("No address found")?
            ))
        }
    }
}

pub enum Stream {
    #[cfg(unix)]
    UNIX(UnixStream),
    TCP(TcpStream)
}

impl Stream {
    pub fn set_ops(&mut self) {
        match self {
            #[cfg(unix)]
            Stream::UNIX(..) => {
                // No corresponding method
            }
            Stream::TCP(s) => {
                s.set_nodelay(true).unwrap();
            }
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => {
                s.read(buf)
            }
            Stream::TCP(s) => {
                s.read(buf)
            }
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => {
                s.write(buf)
            }
            Stream::TCP(s) => {
                s.write(buf)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => {
                s.flush()
            }
            Stream::TCP(s) => {
                s.flush()
            }
        }
    }
}

impl event::Source for Stream {
    fn register(&mut self, registry: &Registry, token: Token, interests: Interest) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => {
                s.register(registry, token, interests)
            }
            Stream::TCP(s) => {
                s.register(registry, token, interests)
            }
        }
    }

    fn reregister(&mut self, registry: &Registry, token: Token, interests: Interest) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => {
                s.reregister(registry, token, interests)
            }
            Stream::TCP(s) => {
                s.reregister(registry, token, interests)
            }
        }
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Stream::UNIX(s) => {
                s.deregister(registry)
            }
            Stream::TCP(s) => {
                s.deregister(registry)
            }
        }
    }
}