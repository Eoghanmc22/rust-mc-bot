mod packet_utils;
mod packet_processors;
mod net;
mod states;
mod sleep;

use std::{net::{TcpStream, ToSocketAddrs}, thread::park, env};
use std::io;
use std::net::SocketAddr;
use rio::Rio;
use crate::packet_processors::PacketProcessor;
use std::sync::Arc;
use crate::net::BotInfo;
use crate::states::{login, play};
use crate::sleep::Sleep;
use rusty_pool::ThreadPool;
use std::time::{Duration, Instant};
use crate::packet_utils::Buf;

const SHOULD_MOVE: bool = true;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        let name = args.get(0).unwrap();
        println!("usage: {} <ip:port> <count> <update time>", name);
        println!("example: {} localhost:25565 500 4", name);
        println!("update time is how long to wait between update cycles.");
        return Ok(());
    }

    let arg1 = args.get(1).unwrap();
    let arg2 = args.get(2).unwrap();
    let arg3 = args.get(3).unwrap();

    let addrs = arg1.to_socket_addrs().expect(&format!("{} is not a ip", arg1)).nth(0).expect(&format!("{} is not a ip", arg1));
    let count: u32 = arg2.parse().expect(&format!("{} is not a number", arg2));
    let millis: u64 = arg3.parse().expect(&format!("{} is not a number", arg3));
    let cpus = 1.max(num_cpus::get()) as u32;

    let mut config = rio::Config::default();
    config.depth = 512;
    let ring = config.start()?;
    let pool = ThreadPool::new(cpus, cpus, Duration::from_secs(3));
    let packet_processor = Arc::new(PacketProcessor::new());

    sleep::start(pool.clone(), millis);

    for i in 0..count {
        println!("spawning bot: {}", i);
        pool.spawn(spawn_bot(ring.clone(), pool.clone(), addrs.clone(), packet_processor.clone(), format!("test{}", i).to_string()));
    }
    loop {
        pool.join();
    }
}

pub async fn spawn_bot(ring: Rio, pool: ThreadPool, addrs: SocketAddr, packet_processor: Arc<PacketProcessor>, name: String) {
    let mut bot = BotInfo {
        ring,
        pool,
        channel: Arc::new(TcpStream::connect(addrs).unwrap()),
        compression_threshold: 0,
        state: 0,
        packet_processor,
        kicked: false
    };
    //login sequence
    BotInfo::send_packet(bot.clone(), login::write_handshake_packet(754, "".to_string(), 0, 2)).await;
    BotInfo::send_packet(bot.clone(), login::write_login_start_packet(&name)).await;
    println!("bot \"{}\" joined", &name);
    let mut time = Instant::now();
    let mut x: f64 = 0.0;
    let mut z : f64 = 0.0;

    //allocate buffer
    let mut packet = Buf::with_length(100000);

    loop {
        if SHOULD_MOVE {
            if time.elapsed().as_millis() > 50 {
                time = Instant::now();
                x += rand::random::<f64>()*2.0-1.0;
                z += rand::random::<f64>()*2.0-1.0;
                BotInfo::send_packet(bot.clone(), play::write_pos(x, 70.0, z, 0.0, 0.0)).await;
            }
        }
        net::process_packet(&mut bot, &mut packet).await;
        if bot.kicked {
            break;
        }
        Sleep::new().await;
    }
}
