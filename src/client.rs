use mio::{Ready, net::TcpStream};
use std::io::Write;
use chatlib::Client;
use std::{io, io::prelude::*};

fn main() {
    let stream = TcpStream::connect(&"127.0.0.1:2203".parse().unwrap()).unwrap();
    let mut client = Client::new(stream);
    loop {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            println!("----");
            client.socket.write(line.unwrap().as_bytes());
        }
    }
}
