use mio::{Events, Token, Poll, Ready, PollOpt, net::TcpListener};
use std::net::SocketAddr;
use chatlib::*;
use std::borrow::{BorrowMut, Borrow};

const LOCAL_ADDR: &str = "127.0.0.1:2203";
const SERVER_TOKEN: Token = Token(0);

struct Miki;

impl Miki {
    fn run(server: &mut Server) {
        println!("miki started");
        let mut events = Events::with_capacity(1024);
        let poll = Poll::new().unwrap();

        poll.register(&server.socket,
                      SERVER_TOKEN,
                      Ready::readable(),
                      PollOpt::edge()
        ).unwrap();

        loop {
            poll.poll(&mut events, None).unwrap();

            for event in events.iter() {
                match event.token() {
                    SERVER_TOKEN => {
                        let mut connection = match server.accept_connection() {
                            Ok(c) => c,
                            Err(ConnectionError::CouldNotAccept(err)) => panic!("{}", err),
                        };
                        let client_token = &connection.token.clone();
                        server.update(connection);

                        if let Some(messages) = server.unread_messages(client_token) {
                            let mut connection = connection.borrow();
                            connection.server_send(messages);
                        }

                        let unregistered = server.last_connection();
                        poll.register(&unregistered.client.socket,
                                      unregistered.token,
                                      Ready::readable(),
                                      PollOpt::edge()
                        ).unwrap();
                    },
                    token => {
                        let read = server.read_from(token);
                        match read {
                            Ok(message) => {
                                match server.direct_send(message) {
                                    Ok(Send::MessageSent(s)) => println!("{}", s),
                                    Err(Send::UserOffline(s)) => println!("{}", s),
                                    Err(Send::SocketWriteError(err)) => panic!("{}", err),
                                    Ok(Send::SocketWriteError(_)) => {}
                                    Ok(Send::UserOffline(_)) => {}
                                    Err(Send::MessageSent(_)) => {}
                                }
                            },
                            Err(Receive::ReceivedZeroBytes(s)) => println!("{}", s),
                            Err(Receive::SocketReadError(err)) => panic!("{}", err),
                        }
                    }
                }
                println!("{} connections", server.connections.len());
            }
        }
    }
}

fn main() {
    let address = LOCAL_ADDR.parse::<SocketAddr>().unwrap();
    let server_socket = TcpListener::bind(&address).unwrap();
    let mut server = Server::new(server_socket);
    Miki::run(&mut server);
}
