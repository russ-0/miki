#![feature(
    proc_macro_hygiene,
    decl_macro,
    register_attr,
    rustc_private,
    type_ascription
)]

#[macro_use]
extern crate rocket;
extern crate ws;

use std::thread;

mod route;
use crate::route::{home};    // TODO: add route files

mod server;
use crate::server::{websocket};


fn rocket() -> rocket::Rocket {
    let routes = routes![];
    rocket::ignite().mount("/", routes)
}

fn main() {
    thread::Builder::new()
        .name("websocket server thread".into())
        .spawn(|| { websocket::new(); })
        .unwrap();

    rocket().launch();
}
