// Copyright (c) 2016 Sandstorm Development Group, Inc.
// Licensed under the MIT License:
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

use gj::{Promise};
use capnp::Error;
use std::cell::Cell;
use std::rc::Rc;
use sandstorm::web_session_capnp::web_session::web_socket_stream;

pub enum Message {
  Text(String),
  Data(Vec<u8>),
}

pub trait MessageHandler {
    fn handle_message(&mut self, message: Message) -> Promise<(), Error>;
}

fn do_ping_pong(client_stream: web_socket_stream::Client,
                timer: ::gjio::Timer,
                awaiting_pong: Rc<Cell<bool>>) -> Promise<(), Error>
{
    let mut req = client_stream.send_bytes_request();
    req.get().set_message(&[0x89, 0]); // PING
    let promise = req.send().promise;
    awaiting_pong.set(true);
    promise.then(move|_| {
        timer.after_delay(::std::time::Duration::new(10, 0)).lift().then(move |_| {
            if awaiting_pong.get() {
                Promise::err(Error::failed("pong not received within 10 seconds".into()))
            } else {
                do_ping_pong(client_stream, timer, awaiting_pong)
            }
        })
    })
}

pub struct Adapter<T> where T: MessageHandler {
    handler: Option<T>,
    awaiting_pong: Rc<Cell<bool>>,
    ping_pong_promise: Promise<(), Error>,
}

impl <T> Adapter<T> where T: MessageHandler {
    pub fn new(handler: T,
               client_stream: web_socket_stream::Client,
               timer: ::gjio::Timer)
               -> Adapter<T> {
        let awaiting = Rc::new(Cell::new(false));
        let ping_pong_promise = do_ping_pong(
            client_stream.clone(),
            timer,
            awaiting.clone()
        ).map_else(|r| match r {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("error while pinging client: {}", e);
                Ok(())
            }
        }).eagerly_evaluate();

        Adapter {
            handler: Some(handler),
            awaiting_pong: awaiting,
            ping_pong_promise: ping_pong_promise,
        }
    }
}

impl <T> web_socket_stream::Server for Adapter<T> where T: MessageHandler {
    fn send_bytes(&mut self,
                  params: web_socket_stream::SendBytesParams,
                  _results: web_socket_stream::SendBytesResults)
                  -> Promise<(), Error>
    {
        let message = pry!(pry!(params.get()).get_message());
        if message.len() < 2 {
            return Promise::err(Error::failed(format!("message must be larger than 2 bytes")))
        }

        let opcode = message[0] & 0xf;
        if message[0] & 0x80 == 0 {
            return Promise::err(Error::failed(format!("non-FIN messages are unsupported")))
        }

        let masked = (message[1] & 0x80) != 0;
        let mut payload_start = 2;
        if masked {
            payload_start += 4;
        }

        let payload_length: usize = match message[1] & 0x7f {
            126 => {
                if message.len() < 4 {
                    return Promise::err(
                        Error::failed(format!("message must be larger than 2 bytes")))
                }
                payload_start += 2;
                (((message[3] as u16) << 8) + message[4] as u16) as usize
            }
            127 => {
                payload_start += 8;
                (((message[3] as u64) << 56) +
                 ((message[4] as u64) << 48) +
                 ((message[5] as u64) << 32) +
                 ((message[6] as u64) << 24) +
                 ((message[7] as u64) << 16) +
                 ((message[8] as u64) << 8) +
                 ((message[9] as u64) << 0)
                ) as usize
            }
            n => n as usize,
        };

        //let mut payload: Vec<u8> = Vec::new();

        println!("length {}, payload_start {}", payload_length, payload_start);

        match opcode {
            0x0 => { // CONTINUE
            }
            0x1 => { // UTF-8 PAYLOAD
            }
            0x2 => { // BINARY PAYLOAD
            }
            0x8 => { // TERMINATE
                self.handler = None;
                self.ping_pong_promise = Promise::ok(())
            }
            0x9 => { // PING
                //TODO
                println!("the client sent us a ping!");
            }
            0xa => { // PONG
                self.awaiting_pong.set(false);
            }
            _ => { // OTHER
                println!("unrecognized websocket opcode {}", opcode);
            }
        }

        &self.handler; // TODO dispatch parsed message to handler
        Promise::ok(())
    }
}
