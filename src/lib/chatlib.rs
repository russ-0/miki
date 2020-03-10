use mio::{Ready, net::TcpStream};
use mio::{Token, net::TcpListener};
use std::time::SystemTime;
use chrono::{MIN_DATE, Date, DateTime, Utc, Duration};
use std::io::{Read, Write};
use std::str;
use std::collections::{HashMap, BinaryHeap};
use std::cell::Cell;
use std::rc::Rc;
use priority_queue::PriorityQueue;
use std::cmp::Ordering;
use std::ops::{Sub, Add};
use std::borrow::Borrow;

#[derive(Debug)]
pub enum ConnectionError {
    CouldNotAccept (String)
}

#[derive(Debug)]
pub enum Receive {
    SocketReadError (String),
    ReceivedZeroBytes (String),
}

#[derive(Debug)]
pub enum Send {
    MessageSent (String),
    SocketWriteError (String),
    UserOffline (String),
}

#[derive(Eq, PartialEq, Hash, Clone)]
pub struct Message {
     //id
     pub timestamp: DateTime<Utc>,
     pub from: Token,
     pub to: Token,
     pub content: String
}

impl Message {
    pub fn generate(from: Token, to: Token, content: String) -> Message {
        Message {
            timestamp: Utc::now(),
            from,
            to,
            content,
        }
    }
}

#[derive(Debug)]
pub struct Client {
    pub socket: TcpStream,
    pub interest: Ready,
}

impl Client {
    pub fn new(socket: TcpStream) -> Client {
        Client {
            socket,
            interest: Ready::readable(),
        }
    }
}

pub struct UnreadCache {
    size: usize,
    pub capacity: usize,
    pub lifetime: u32,
    unordered: HashMap<Token, Vec<Message>>,
    ordered: PriorityQueue<Message, DateTime<Utc>>,
}

impl UnreadCache {

    fn schedule_for_expiration(&mut self, messages: Vec<Message>) {
        messages.iter().map(|message| self.ordered.change_priority(
            &message,
            MIN_DATE.and_hms(0, 0, 0)
        ));
    }

    pub fn remove(&mut self, token: &Token) {
        let messages = match self.get_unread(token) {
            Some(m) => m.clone(),
            None => return
        };
        self.size -= messages.len();
        self.schedule_for_expiration(messages);
        self.expire(None);
        self.unordered.remove(token);
    }

    pub fn insert(&mut self, unread: Message) {
        if self.size >= self.capacity {
            self.archive();
        }

        let timestamp = unread.timestamp.clone();
        let indexed = unread.clone();
        self.ordered.push(unread, timestamp);

        println!("cached message: {}", &indexed.content);

        self.size += 1;
        let token = &indexed.to;
        if self.unordered.contains_key(token) {
            let unreads = self.get_unread(token).unwrap();
            unreads.push(indexed);
        } else {
            self.unordered.insert(*token, vec![indexed]);
        }
    }

    fn archive(&mut self) {}

    pub fn get_unread(&mut self, token: &Token) -> Option<&mut Vec<Message>> {
        self.unordered.get_mut(token)
    }

    pub fn extract(&self, token: &Token) -> Option<&Vec<Message>> {
        self.unordered.get(token)
    }

    pub fn expire(&mut self, duration: Option<Duration>) {
        while let Some(message) = self.ordered.peek() {
            if let Some(duration) = duration {
                if *message.1 + duration < Utc::now() {
                    self.ordered.pop().unwrap();
                } else { break }
            } else {
                if message.1 == &MIN_DATE.and_hms(0, 0, 0) {
                    self.ordered.pop().unwrap();
                } else { break }
            }
        }
    }

    pub fn empty(&self) -> bool {
        self.size == 0 && self.unordered.is_empty()
    }
}

#[derive(Debug)]
pub struct Connection {
    pub token: Token,
    pub client: Client,
}

impl Connection {
    pub fn server_send(&mut self, messages: &Vec<Message>) -> Vec<Result<(), Send>> {
        let mut send_results = Vec::new();
        for message in messages.iter() {
            match self.client.socket.write(&message.content.as_bytes()) {
                Ok(_) => send_results.push(Ok(())),
                Err(e) => send_results.push(Err(Send::SocketWriteError(e.to_string())))
            };
        }

        send_results
    }
}

mod parser {
    pub fn newline_strip(line: &mut String) {       // TODO: replace while loop with iterator
        while line.ends_with('\n') || line.ends_with('\r') || line.ends_with('\u{0}') {
            line.pop();
        }
    }
}

pub struct Server {
    pub socket: TcpListener,
    pub connections: HashMap<Token, Connection>,
    pub token_counter: usize,
    pub connection_order: Vec<Token>,
    unread_messages: UnreadCache,
}

impl Server {
    pub fn new(socket: TcpListener) -> Server {
        Server {
            socket,
            connections: HashMap::new(),
            connection_order: Vec::new(),
            token_counter: 1,
            unread_messages: UnreadCache {
                size: 0,
                capacity: 100,
                lifetime: 300,
                unordered: HashMap::new(),
                ordered: PriorityQueue::new()
            }
        }
    }

    pub fn update(&mut self, connection: Connection) {
        self.connections.insert(connection.token, connection);
    }

    pub fn accept_connection(&mut self) -> Result<Connection, ConnectionError> {
        match self.socket.accept() {
            Ok((stream, _client_addr)) => {
                let token = Token(self.token_counter);
                let client = Client::new(stream);
                self.token_counter = self.token_counter + 1;
                self.connection_order.push(token.clone());
                let connection = Connection {token, client};
                println!("new connection: {:?}", connection);
                Ok(connection)
            },
            Err(e) => Err(ConnectionError::CouldNotAccept(e.to_string()))
        }
    }

    fn generate_message(&mut self, from: Token, buffer: Vec<u8>) -> Message {
        let data = str::from_utf8(&buffer).unwrap();
        let components = data.split("~").collect::<Vec<&str>>();
        let to = Token(components[0].parse().unwrap());
        let mut content = components[1].to_string();
        parser::newline_strip(&mut content);
        Message::generate(from, to, content.to_string())
    }

    pub fn last_connection(&self) -> &Connection {
        let token = self.connection_order.last().unwrap();
        self.connections.get(token).unwrap()
    }

    pub fn read_from(&mut self, token: Token) -> Result<Message, Receive> {
        let mut buffer = vec![0 as u8; 1024];
        let connection = self.connections.get_mut(&token).unwrap();
        let read = connection.client.socket.read(&mut buffer);

        match read {
            Ok(0) => {
                self.connections.remove(&token);
                Err(Receive::ReceivedZeroBytes(format!("{:?} disconnected", token)))
            },
            Ok(len) => {
                let message = self.generate_message(token, buffer);
                println!("received {} bytes: {}", len, message.content);
                Ok(message)
            },
            Err(e) => Err(Receive::SocketReadError(e.to_string()))
        }
    }

    pub fn direct_send(&mut self, message: Message) -> Result<Send, Send> {
        if let Some(to) = self.connections.get_mut(&message.to) {
            match to.client.socket.write(&message.content.as_bytes()) {
                Ok(_) => Ok(Send::MessageSent(format!("sent message: {:?}", &message.content))),
                Err(e) => Err(Send::SocketWriteError(e.to_string()))
            }
        } else {
            let debug_message = format!("{:?} is offline", &message.to);
            self.unread_messages.insert(message.clone());
            Err(Send::UserOffline(debug_message))
        }
    }

    pub fn unread_messages(&mut self, token: &Token) -> Option<&Vec<Message>> {
        self.unread_messages.extract(token)
    }

    pub fn send_unreads(&mut self, messages: &Vec<Message>) {
        messages.iter().map(|message| self.direct_send(message.clone()));
    }
}
