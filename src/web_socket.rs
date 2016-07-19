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

enum PreviousFrames {
   None,
   Text(String),
   Data(Vec<u8>)
}

#[derive(Debug)]
enum ParserState {
    NotStarted,
    DoneFirstByte { fin: bool, opcode: u8},
    ReadingLongPayloadLength {fin: bool, opcode: u8, masked: bool,
                            payload_len_bytes_read: usize, payload_len_so_far: u64 },
    ReadingMask { fin: bool, opcode: u8, mask_bytes_read: usize, payload_len: u64,
                  mask_so_far: [u8; 4] },
    ReadingPayload { fin: bool, opcode: u8, payload_len: u64, mask: [u8; 4], bytes_so_far: Vec<u8> },
}

struct ParseResult {
    frame: Vec<u8>,
    opcode: u8,
    fin: bool,
}

impl ParserState {
    fn done_payload_length(bytes_read: usize,
                           fin: bool, opcode: u8, masked: bool, payload_len: u64)
                           -> (ParserState, (usize, Option<ParseResult>))
    {
        use self::ParserState::*;
        if masked {
            (ReadingMask { fin: fin, opcode: opcode, payload_len: payload_len,
                           mask_bytes_read: 0, mask_so_far: [0; 4] },
             (bytes_read, None))
        } else if payload_len == 0 {
            (NotStarted,
             (bytes_read, Some(ParseResult { frame: Vec::new(), fin: fin, opcode: opcode })))
        } else {
            (ReadingPayload { fin: fin, opcode: opcode,
                              payload_len: payload_len, mask: [0; 4],
                              bytes_so_far: Vec::new() },
              (bytes_read, None))
        }
    }


    /// returns number of bytes consumed and the complete message, if there is one.
    fn advance(&mut self, buf: &[u8]) -> (usize, Option<ParseResult>) {
        use self::ParserState::*;
        let (new_state, result) = match self {
            &mut NotStarted => {
                if buf.len() < 1 {
                    return (0, None)
                }

                (DoneFirstByte { fin: (buf[0] & 0x80) != 0, opcode: buf[0] & 0xf }, (1, None))
            }
            &mut DoneFirstByte { fin, opcode } => {
                if buf.len() < 1 {
                    return (0, None)
                }

                let masked = (buf[0] & 0x80) != 0;

                match buf[0] & 0x7f {
                    126 => {
                        (ReadingLongPayloadLength {
                            fin: fin,
                            opcode: opcode,
                            masked: masked,
                            payload_len_bytes_read: 6,
                            payload_len_so_far: 0,
                        }, (1, None))
                    }
                    127 => {
                        (ReadingLongPayloadLength {
                            fin: fin,
                            opcode: opcode,
                            masked: masked,
                            payload_len_bytes_read: 0,
                            payload_len_so_far: 0,
                        }, (1, None))
                    }
                    n => ParserState::done_payload_length(1, fin, opcode, masked, n as u64)
                }
            }
            &mut ReadingLongPayloadLength { fin, opcode, masked, payload_len_bytes_read,
                                            payload_len_so_far } => {
                let mut idx = 0;
                let mut new_so_far = payload_len_so_far;
                while idx + payload_len_bytes_read < 8 && idx < buf.len() {
                    new_so_far += (buf[idx] as u64) << (8 * (7 - idx - payload_len_bytes_read));
                    idx += 1;
                }

                if buf.len() + payload_len_bytes_read < 8 {
                    (ReadingLongPayloadLength {
                        fin: fin,
                        opcode: opcode,
                        masked: masked,
                        payload_len_bytes_read: idx + payload_len_bytes_read,
                        payload_len_so_far: new_so_far,
                    }, (idx, None))
                } else {
                    ParserState::done_payload_length(idx, fin, opcode, masked, new_so_far)
                }
            }
            &mut ReadingMask { fin, opcode, mask_bytes_read, payload_len, mask_so_far } => {
                let mut idx = 0;
                let mut new_so_far = mask_so_far;
                while idx + mask_bytes_read < 4 && idx < buf.len() {
                    new_so_far[idx] = buf[idx];
                    idx += 1;
                }

                if buf.len() + mask_bytes_read < 4 {
                    (ReadingMask {
                        fin: fin,
                        opcode: opcode,
                        payload_len: payload_len,
                        mask_bytes_read: idx + mask_bytes_read,
                        mask_so_far: new_so_far,
                    }, (idx, None))
                } else if payload_len == 0 {
                    (NotStarted,
                     (idx, Some(ParseResult { frame: Vec::new(), fin: fin, opcode: opcode })))
                } else {
                    (ReadingPayload { fin: fin, opcode: opcode, mask: new_so_far,
                                      bytes_so_far: Vec::new(),
                                      payload_len: payload_len },
                     (idx, None))
                }
            }
            &mut ReadingPayload { fin, opcode, payload_len, mask, ref mut bytes_so_far } => {
                let mut idx = 0;

                while (bytes_so_far.len() as u64) < payload_len && idx < buf.len() {
                    let mask_byte = mask[bytes_so_far.len() % 4];
                    bytes_so_far.push(buf[idx] ^ mask_byte);
                    idx += 1;
                }

                if (bytes_so_far.len() as u64) < payload_len {
                    return (idx, None)
                } else {
                    let frame = ::std::mem::replace(bytes_so_far, Vec::new());
                    (NotStarted,
                     (idx, Some(ParseResult { frame: frame, fin: fin, opcode: opcode })))
                }

            }
        };

        *self = new_state;
        result
    }
}

pub struct Adapter<T> where T: MessageHandler {
    handler: Option<T>,
    awaiting_pong: Rc<Cell<bool>>,
    ping_pong_promise: Promise<(), Error>,
    parser_state: ParserState,
    previous_frames: PreviousFrames,
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
            parser_state: ParserState::NotStarted,
            previous_frames: PreviousFrames::None,
        }
    }

    fn process_message(&mut self) -> Promise<(), Error> {
        let frames = ::std::mem::replace(&mut self.previous_frames,
                                         PreviousFrames::None);
        let message = match frames {
            PreviousFrames::None => {
                return Promise::err(Error::failed(format!("message has no frames")));
            }
            PreviousFrames::Data(d) => {
                Message::Data(d)
            }
            PreviousFrames::Text(t) => {
                Message::Text(t)
            }
        };

        match self.handler {
            Some(ref mut h) => h.handle_message(message),
            None => Promise::ok(()),
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
        let mut result_promises = Vec::<Promise<(), Error>>::new();
        let mut num_bytes_read = 0;
        while num_bytes_read < message.len() {
            let (n, result) = self.parser_state.advance(&message[num_bytes_read..]);
            num_bytes_read += n;
            match result {
                None => (),
                Some(ParseResult { frame, opcode, fin }) => {
                    match opcode {
                        0x0 => { // CONTINUE
                            match &mut self.previous_frames {
                                &mut PreviousFrames::None => {
                                    return Promise::err(Error::failed(
                                        format!("CONTINUE frame received, but there are no \
                                                 previous frames.")));
                                }
                                &mut PreviousFrames::Data(ref mut data) => {
                                    data.extend_from_slice(&frame[..])
                                }
                                &mut PreviousFrames::Text(ref mut text) => {
                                    text.push_str(&pry!(String::from_utf8(frame)))
                                }
                            }

                            if fin {
                                result_promises.push(self.process_message());
                            }
                        }
                        0x1 => { // UTF-8 PAYLOAD
                            self.previous_frames =
                                PreviousFrames::Text(pry!(String::from_utf8(frame)));

                            if fin {
                                result_promises.push(self.process_message());
                            }
                        }
                        0x2 => { // BINARY PAYLOAD
                            self.previous_frames = PreviousFrames::Data(frame);

                            if fin {
                                result_promises.push(self.process_message());
                            }
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
                }
            }
        }

        Promise::all(result_promises.into_iter()).map(|_| Ok(()))
    }
}
