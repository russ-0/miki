use ws::{
    listen,
    CloseCode,
    Error,
    Handler,
    Handshake,
    Message,
    Request,
    Response,
    Result,
    Sender,
};

use std::cell::Cell;
use std::rc::Rc;

struct Server {
    output: Sender,
    conn_count: Rc<Cell<u32>>,
}

impl Server {
    fn new(output: Sender) -> Server {
        Server {
            output,
            conn_count: Rc::new(Cell::new(0)),
        }
    }
}

impl Handler for Server {
}

pub fn new() -> () {        // create new websocket for listening
    listen("127.0.0.1:2203", |output| { Server::new(output) }).unwrap()
}
