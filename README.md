# `rust-mc-bot`

A high performance stress testing tool for custom Minecraft server implementations

## Motivation

When I was working on the [Minestom Project](https://github.com/Minestom/Minestom) we needed a reliable way to verify the end-to-end performance of our code but all the existing bot implementations would crash before the server did. So I made a bot implementation dedicated to having the best performance possible (and least unnecessary features).

## Usage

1. Clone the code
    - If you want to stress test a server implementing an older version of the minecraft protocol the corresponding tag
    - ```bash
      git checkout tags/1.18.1
      ```
2. Compile the code
    - Make sure Rust is installed. See [here](https://rustup.rs).
    - ```bash
      cargo build --release
      ```
    - Executable will be built to `target/release/rust-mc-bot` (linux/macos) or `target/release/rust-mc-bot.exe` (Windows)
3. Start the bots
    - Usage:
      ```bash
      ./rust-mc-bot <ip:port or path> <count> [threads]
      ./rust-mc-bot 127.0.0.1:25565 1000
      ```
    - Alternative:
      ```bash
      cargo run --release -- <ip:port or path> <count> [threads]
      cargo run --release -- 127.0.0.1:25565 1000
      ```

## Known Issues

Using `localhost` as the IP on machines with ipv6 may cause the bots to not connect to the server. Please use `127.0.0.1` instead.

Currently, the bots do not support online mode to prevent abuse and to improve performance.

## Disclaimer

This should **ONLY** be used test your own server. We do not endorse the use of this for any other purposes than testing your own infrastructure.

Please be aware that attempting to execute this with an external server as a target can be seen as **illegal** as it simulates a layer 7 DoS (denial-of-service) attack, which is against the law in most countries.

