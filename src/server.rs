// Copyright (c) 2014-2016 Sandstorm Development Group, Inc.
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

use gj::{Promise, EventLoop};
use capnp::Error;
use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};
use rustc_serialize::{base64, hex};

use std::collections::hash_map::HashMap;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use collections_capnp::ui_view_metadata;

use sandstorm::powerbox_capnp::powerbox_descriptor;
use sandstorm::grain_capnp::{session_context, user_info, ui_view, ui_session, sandstorm_api};
use sandstorm::grain_capnp::{static_asset};
use sandstorm::web_session_capnp::{web_session};
use sandstorm::web_session_capnp::web_session::web_socket_stream;

pub struct WebSocketStream {
    id: u64,
    awaiting_pong: Rc<Cell<bool>>,
    _ping_pong_promise: Promise<(), Error>,
    saved_ui_views: Rc<RefCell<SavedUiViewSet>>,
}

impl Drop for WebSocketStream {
    fn drop(&mut self) {
        self.saved_ui_views.borrow_mut().subscribers.remove(&self.id);
    }
}

fn do_ping_pong(client_stream: web_socket_stream::Client,
                timer: ::gjio::Timer,
                awaiting_pong: Rc<Cell<bool>>) -> Promise<(), Error>
{
    println!("pinging");
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


impl WebSocketStream {
    fn new(id: u64,
           client_stream: web_socket_stream::Client,
           timer: ::gjio::Timer,
           saved_ui_views: Rc<RefCell<SavedUiViewSet>>)
           -> WebSocketStream
    {
        let awaiting = Rc::new(Cell::new(false));
        let ping_pong_promise = do_ping_pong(client_stream,
                                             timer,
                                             awaiting.clone()).map_else(|r| match r {
            Ok(_) => Ok(()),
            Err(e) => {println!("ERROR {}", e); Ok(())  }
        }).eagerly_evaluate();

        WebSocketStream {
            id: id,
            awaiting_pong: awaiting,
            _ping_pong_promise: ping_pong_promise,
            saved_ui_views: saved_ui_views,
        }
    }
}

impl web_socket_stream::Server for WebSocketStream {
    fn send_bytes(&mut self,
                  params: web_socket_stream::SendBytesParams,
                  _results: web_socket_stream::SendBytesResults)
                  -> Promise<(), Error>
    {
        let message = pry!(pry!(params.get()).get_message());
        let opcode = message[0] & 0xf; // or is it 0xf0?
        let masked = (message[1] & 0x80) != 0;
        let length = message[1] & 0x7f;

        match opcode {
            0x0 => { // CONTINUE
            }
            0x1 => { // UTF-8 PAYLOAD
            }
            0x2 => { // BINARY PAYLOAD
            }
            0x8 => { // TERMINATE
                // TODO: drop things to get them to close.
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
        println!("opcode {}, masked {}, length {}", opcode, masked, length);
        println!("websocket message {:?}", message);
        Promise::ok(())
    }
}

fn encode_websocket_message(mut params: web_socket_stream::send_bytes_params::Builder,
                            message: &str)
{
    // TODO(perf) avoid this allocation
    let mut bytes: Vec<u8> = Vec::new();
    bytes.push(0x81);
    if message.len() < 126 {
        bytes.push(message.len() as u8);
    } else if message.len() < 1 << 16  {
        // 16 bits
        bytes.push(0x7e);
        bytes.push((message.len() >> 8) as u8);
        bytes.push(message.len() as u8);
    } else {
        // 64 bits
        bytes.push(0x7f);
        bytes.push((message.len() >> 56) as u8);
        bytes.push((message.len() >> 48) as u8);
        bytes.push((message.len() >> 40) as u8);
        bytes.push((message.len() >> 32) as u8);
        bytes.push((message.len() >> 16) as u8);
        bytes.push((message.len() >> 24) as u8);
        bytes.push((message.len() >> 8) as u8);
        bytes.push(message.len() as u8);
    }

    bytes.extend_from_slice(message.as_bytes());

    params.set_message(&bytes[..]);
}

#[derive(Clone)]
struct SavedUiViewData {
    title: String,
    date_added: u64,
    added_by: String,
}

impl SavedUiViewData {
    fn to_json(&self) -> String {
        format!("{{\"title\":\"{}\",\"date_added\": \"{}\",\"added_by\":\"{}\"}}",
                self.title,
                self.date_added,
                self.added_by)
    }
}

#[derive(Clone)]
struct ViewInfoData {
    app_title: String,
    grain_icon_url: String,
}

impl ViewInfoData {
    fn to_json(&self) -> String {
        format!("{{\"app_title\":\"{}\",\"grain_icon_url\":\"{}\"}}",
                self.app_title, self.grain_icon_url)
    }
}

#[derive(Clone)]
enum Action {
    Insert { token: String, data: SavedUiViewData },
    Remove { token: String },
    ViewInfo { token: String, data: ViewInfoData },
    CanWrite(bool),
    Description(String),
}

impl Action {
    fn to_json(&self) -> String {
        match self {
            &Action::Insert { ref token, ref data } => {
                format!("{{\"insert\":{{\"token\":\"{}\",\"data\":{} }} }}",
                        token, data.to_json())
            }
            &Action::Remove { ref token } => {
                format!("{{\"remove\":{{\"token\":\"{}\"}}}}", token)
            }
            &Action::ViewInfo { ref token, ref data } => {
                format!("{{\"viewInfo\":{{\"token\":\"{}\",\"data\":{} }} }}",
                        token, data.to_json())
            }
            &Action::CanWrite(b) => {
                format!("{{\"canWrite\":{}}}", b)
            }
            &Action::Description(ref s) => {
                format!("{{\"description\":\"{}\"}}", s)
            }
        }
    }
}

struct Reaper;

impl ::gj::TaskReaper<(), Error> for Reaper {
    fn task_failed(&mut self, error: Error) {
        // TODO better message.
        println!("task failed: {}", error);
    }
}

pub struct SavedUiViewSet {
    base_path: ::std::path::PathBuf,
    views: HashMap<String, SavedUiViewData>,
    view_infos: HashMap<String, ViewInfoData>,
    next_id: u64,
    subscribers: HashMap<u64, web_socket_stream::Client>,
    tasks: ::gj::TaskSet<(), Error>,
    description: String,
    sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned>,
}

impl SavedUiViewSet {
    pub fn new<P>(token_directory: P,
                  sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned>)
                  -> ::capnp::Result<Rc<RefCell<SavedUiViewSet>>>
        where P: AsRef<::std::path::Path>
    {
        let description = match ::std::fs::File::open("/var/description") {
            Ok(mut f) => {
                use std::io::Read;
                let mut result = String::new();
                try!(f.read_to_string(&mut result));
                result
            }
            Err(ref e) if e.kind() == ::std::io::ErrorKind::NotFound => {
                use std::io::Write;
                let mut f = try!(::std::fs::File::create("/var/description"));
                let result = "this is a description";
                try!(f.write_all(result.as_bytes()));
                result.into()
            }
            Err(e) => {
                return Err(e.into());
            }
        };

        let result = Rc::new(RefCell::new(SavedUiViewSet {
            base_path: token_directory.as_ref().to_path_buf(),
            views: HashMap::new(),
            view_infos: HashMap::new(),
            next_id: 0,
            subscribers: HashMap::new(),
            tasks: ::gj::TaskSet::new(Box::new(Reaper)),
            description: description,
            sandstorm_api: sandstorm_api,
        }));

        // create token directory if it does not yet exist
        try!(::std::fs::create_dir_all(&token_directory));

        for token_file in try!(::std::fs::read_dir(&token_directory)) {
            let dir_entry = try!(token_file);
            let token: String = match dir_entry.file_name().to_str() {
                None => {
                    println!("malformed token: {:?}", dir_entry.file_name());
                    continue
                }
                Some(s) => s.into(),
            };

            let mut reader = try!(::std::fs::File::open(dir_entry.path()));
            let message = try!(::capnp::serialize::read_message(&mut reader, Default::default()));
            let metadata: ui_view_metadata::Reader = try!(message.get_root());

            let entry = SavedUiViewData {
                title: try!(metadata.get_title()).into(),
                date_added: metadata.get_date_added(),
                added_by: try!(metadata.get_added_by()).into(),
            };

            result.borrow_mut().views.insert(token.clone(), entry);

            try!(SavedUiViewSet::retrieve_view_info(&result, token));
        }

        Ok(result)
    }

    fn retrieve_view_info(set_ref: &Rc<RefCell<SavedUiViewSet>>,
                          token: String) -> ::capnp::Result<()> {
        // SandstormApi.restore, then call getViewInfo,
        // then call get_url() on the grain static asset.

        let set = set_ref.clone();
        let binary_token = match base64::FromBase64::from_base64(&token[..]) {
            Ok(b) => b,
            Err(e) => return Err(Error::failed(format!("{}", e))),
        };

        let mut req = set.borrow().sandstorm_api.restore_request();
        req.get().set_token(&binary_token);
        let task = req.send().promise.then(move |response| {
            let view: ui_view::Client =
                pry!(pry!(response.get()).get_cap().get_as_capability());
            view.get_view_info_request().send().promise.then(move |response| {
                let view_info = pry!(response.get());
                let app_title = pry!(pry!(view_info.get_app_title()).get_default_text()).to_string();
                let asset = pry!(view_info.get_grain_icon());
                asset.get_url_request().send().promise.then(move |response| {
                    let result = pry!(response.get());
                    let protocol = match pry!(result.get_protocol()) {
                        static_asset::Protocol::Https => "https".to_string(),
                        static_asset::Protocol::Http => "http".to_string(),
                    };

                    let info = ViewInfoData {
                        app_title: app_title,
                        grain_icon_url: format!("{}://{}", protocol, pry!(result.get_host_path())),
                    };

                    set.borrow_mut().view_infos.insert(token.clone(), info.clone());

                    let json_string = Action::ViewInfo { token: token, data: info }.to_json();
                    set.borrow_mut().send_message_to_subscribers(&json_string);

                    Promise::ok(())
                })
            })
        });

        set_ref.borrow_mut().tasks.add(task);
        Ok(())
    }

    fn update_description(&mut self, description: &[u8]) -> ::capnp::Result<()> {
        use std::io::Write;

        let desc_string: String = match ::std::str::from_utf8(description) {
            Err(e) => return Err(::capnp::Error::failed(format!("{}", e))),
            Ok(d) => d.into(),
        };

        let temp_path = format!("/var/description.uploading");
        try!(try!(::std::fs::File::create(&temp_path)).write_all(description));
        try!(::std::fs::rename(temp_path, "/var/description"));


        self.description = desc_string;

        let json_string = Action::Description(self.description.clone()).to_json();
        self.send_message_to_subscribers(&json_string);
        Ok(())
    }

    fn insert(&mut self,
              token: String,
              title: String,
              added_by: String) -> ::capnp::Result<()> {
        let dur = ::std::time::SystemTime::now().duration_since(::std::time::UNIX_EPOCH).expect("TODO");
        let date_added = dur.as_secs() * 1000 + (dur.subsec_nanos() / 1000000) as u64;

        let mut token_path = ::std::path::PathBuf::new();
        token_path.push(self.base_path.clone());
        token_path.push(token.clone());
        let mut writer = try!(::std::fs::File::create(token_path));

        let mut message = ::capnp::message::Builder::new_default();
        {
            let mut metadata: ui_view_metadata::Builder = message.init_root();
            metadata.set_title(&title);
            metadata.set_date_added(date_added);
            metadata.set_added_by(&added_by);
        }

        try!(::capnp::serialize::write_message(&mut writer, &message));

        let entry = SavedUiViewData {
            title: title,
            date_added: date_added,
            added_by: added_by,
        };

        let json_string = Action::Insert { token: token.clone(), data: entry.clone() }.to_json();
        self.send_message_to_subscribers(&json_string);
        self.views.insert(token, entry);

        Ok(())
    }

    fn send_message_to_subscribers(&mut self, message: &str) {
        for (_, sub) in &self.subscribers {
            let mut req = sub.send_bytes_request();
            encode_websocket_message(req.get(), message);
            self.tasks.add(req.send().promise.map(|_| Ok(())));
        }
    }

    fn remove(&mut self, token: &str) -> Result<(), Error> {
        if let Err(e) = ::std::fs::remove_file(format!("/var/sturdyrefs/{}", token)) {
            if e.kind() != ::std::io::ErrorKind::NotFound {
                return Err(e.into())
            }
        }

        let json_string = Action::Remove { token: token.into() }.to_json();
        self.send_message_to_subscribers(&json_string);
        self.views.remove(token);
        Ok(())
    }


    fn new_subscribed_websocket(set: &Rc<RefCell<SavedUiViewSet>>,
                                client_stream: web_socket_stream::Client,
                                can_write: bool,
                                timer: &::gjio::Timer)
                                 -> WebSocketStream
    {
        let id = set.borrow().next_id;
        set.borrow_mut().next_id = id + 1;

        set.borrow_mut().subscribers.insert(id, client_stream.clone());

        let mut task = Promise::ok(());

        {
            let json_string = Action::CanWrite(can_write).to_json();
            let mut req = client_stream.send_bytes_request();
            encode_websocket_message(req.get(), &json_string);
            let promise = req.send().promise.map(|_| Ok(()));
            task = task.then(|_| promise);
        }

        {
            let json_string = Action::Description(set.borrow().description.clone()).to_json();
            let mut req = client_stream.send_bytes_request();
            encode_websocket_message(req.get(), &json_string);
            let promise = req.send().promise.map(|_| Ok(()));
            task = task.then(|_| promise);
        }

        for (t, v) in &set.borrow().views {
            let action = Action::Insert {
                token: t.clone(),
                data: v.clone()
            };

            let json_string = action.to_json();
            let mut req = client_stream.send_bytes_request();
            encode_websocket_message(req.get(), &json_string);
            let promise = req.send().promise.map(|_| Ok(()));
            task = task.then(|_| promise);
        }

        for (t, vi) in &set.borrow().view_infos {
            let action = Action::ViewInfo {
                token: t.clone(),
                data: vi.clone()
            };

            let json_string = action.to_json();
            let mut req = client_stream.send_bytes_request();
            encode_websocket_message(req.get(), &json_string);
            let promise = req.send().promise.map(|_| Ok(()));
            task = task.then(|_| promise);
        }

        set.borrow_mut().tasks.add(task);

        WebSocketStream::new(id, client_stream, timer.clone(), set.clone())
    }
}

pub struct WebSession {
    timer: ::gjio::Timer,
    can_write: bool,
    sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned>,
    context: session_context::Client,
    saved_ui_views: Rc<RefCell<SavedUiViewSet>>,
    identity_id: String,
}

impl WebSession {
    pub fn new(timer: ::gjio::Timer,
               user_info: user_info::Reader,
               context: session_context::Client,
               _params: web_session::params::Reader,
               sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned>,
               saved_ui_views: Rc<RefCell<SavedUiViewSet>>)
               -> ::capnp::Result<WebSession>
    {
        // Permission #0 is "write". Check if bit 0 in the PermissionSet is set.
        let permissions = try!(user_info.get_permissions());
        let can_write = permissions.len() > 0 && permissions.get(0);

        Ok(WebSession {
            timer: timer,
            can_write: can_write,
            sandstorm_api: sandstorm_api,
            context: context,
            saved_ui_views: saved_ui_views,
            identity_id: hex::ToHex::to_hex(try!(user_info.get_identity_id())),
        })

        // `UserInfo` is defined in `sandstorm/grain.capnp` and contains info like:
        // - A stable ID for the user, so you can correlate sessions from the same user.
        // - The user's display name, e.g. "Mark Miller", useful for identifying the user to other
        //   users.
        // - The user's permissions (seen above).

        // `WebSession::Params` is defined in `sandstorm/web-session.capnp` and contains info like:
        // - The hostname where the grain was mapped for this user. Every time a user opens a grain,
        //   it is mapped at a new random hostname for security reasons.
        // - The user's User-Agent and Accept-Languages headers.

        // `SessionContext` is defined in `sandstorm/grain.capnp` and implements callbacks for
        // sharing/access control and service publishing/discovery.
    }
}

impl ui_session::Server for WebSession {}

impl web_session::Server for WebSession {
    fn get(&mut self,
           params: web_session::GetParams,
           mut results: web_session::GetResults)
	-> Promise<(), Error>
    {
        // HTTP GET request.
        let path = pry!(pry!(params.get()).get_path());
        pry!(self.require_canonical_path(path));

        println!("PATH {}", path);

        if path == "" {
            let text = "<!DOCTYPE html>\
                       <html><head>\
                       <link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\">\
                       <script type=\"text/javascript\" src=\"script.js\" async></script>
                       </head><body><div id=\"main\"></div></body></html>";
            let mut content = results.get().init_content();
            content.set_mime_type("text/html; charset=UTF-8");
            content.init_body().set_bytes(text.as_bytes());
            Promise::ok(())
        } else if path == "script.js" {
            self.read_file("/script.js.gz", results, "text/javascript; charset=UTF-8", Some("gzip"))
        } else if path == "style.css" {
            self.read_file("/style.css.gz", results, "text/css; charset=UTF-8", Some("gzip"))
        } else {
            let mut error = results.get().init_client_error();
            error.set_status_code(web_session::response::ClientErrorCode::NotFound);
            Promise::ok(())
        }
    }

    fn post(&mut self,
            params: web_session::PostParams,
            mut results: web_session::PostResults)
            -> Promise<(), Error>
    {
        let path = {
            let path = pry!(pry!(params.get()).get_path());
            pry!(self.require_canonical_path(path));
            path.to_string()
        };

        if path.starts_with("token/") {
            self.receive_request_token(
                path[6..].to_string(),
                params, results)
        } else if path.starts_with("offer/") {
            self.offer_ui_view(path[6..].to_string(), params, results)
        } else {
            let mut error = results.get().init_client_error();
            error.set_status_code(web_session::response::ClientErrorCode::NotFound);
            Promise::ok(())
        }
    }

    fn put(&mut self,
           params: web_session::PutParams,
           mut results: web_session::PutResults)
	-> Promise<(), Error>
    {
        // HTTP PUT request.

        let params = pry!(params.get());
        let path = pry!(params.get_path());
        pry!(self.require_canonical_path(path));

        if !self.can_write {
            results.get().init_client_error()
                .set_status_code(web_session::response::ClientErrorCode::Forbidden);
        } else if path == "description" {
            let content = pry!(pry!(params.get_content()).get_content());
            pry!(self.saved_ui_views.borrow_mut().update_description(content));
            results.get().init_no_content();
        }
        Promise::ok(())
    }

    fn delete(&mut self,
              params: web_session::DeleteParams,
              mut results: web_session::DeleteResults)
	-> Promise<(), Error>
    {
        // HTTP DELETE request.

        let path = pry!(pry!(params.get()).get_path());
        pry!(self.require_canonical_path(path));

        if !path.starts_with("sturdyref/") {
            return Promise::err(Error::failed("DELETE only supported under sturdyref/".to_string()));
        }

        if !self.can_write {
            results.get().init_client_error()
                .set_status_code(web_session::response::ClientErrorCode::Forbidden);
            Promise::ok(())
        } else {
            pry!(self.saved_ui_views.borrow_mut().remove(&path[10..]));
            results.get().init_no_content();
            Promise::ok(())
        }
    }

    fn open_web_socket(&mut self,
                     params: web_session::OpenWebSocketParams,
                     mut results: web_session::OpenWebSocketResults)
                     -> Promise<(), Error>
    {
        println!("open web socket!");
        let client_stream = pry!(pry!(params.get()).get_client_stream());


        results.get().set_server_stream(
            web_socket_stream::ToClient::new(
                SavedUiViewSet::new_subscribed_websocket(
                    &self.saved_ui_views,
                    client_stream,
                    self.can_write,
                    &self.timer)).from_server::<::capnp_rpc::Server>());

        Promise::ok(())
    }
}

impl WebSession {
    fn offer_ui_view(&mut self,
                     text_token: String,
                     _params: web_session::PostParams,
                     mut results: web_session::PostResults)
                     -> Promise<(), Error>
    {
        let token = match base64::FromBase64::from_base64(&text_token[..]) {
            Ok(b) => b,
            Err(e) => return Promise::err(Error::failed(format!("{}", e))),
        };

        let session_context = self.context.clone();
        let mut req = self.sandstorm_api.restore_request();
        req.get().set_token(&token);
        req.send().promise.then(move |response| {
            let sealed_ui_view: ui_view::Client =
                pry!(pry!(response.get()).get_cap().get_as_capability());
            let mut req = session_context.offer_request();
            req.get().get_cap().set_as_capability(sealed_ui_view.client.hook);
            {
                use capnp::traits::HasTypeId;
                let tags = req.get().init_descriptor().init_tags(1);
                tags.get(0).set_id(ui_view::Client::type_id());
            }

            req.send().promise
        }).then_else(move |r| match r {
            Ok(_) => {
                results.get().init_no_content();
                Promise::ok(())
            }
            Err(e) => {
                let mut client_error = results.get().init_client_error();
                client_error.set_description_html(&format!("{}", e)[..]);
                Promise::ok(())
            }
        })
    }

    fn receive_request_token(&mut self,
                             token: String,
                             params: web_session::PostParams,
                             mut results: web_session::PostResults)
                             -> Promise<(), Error>
    {
        println!("token: {}", token);

        let content = pry!(pry!(pry!(params.get()).get_content()).get_content());

        let decoded_content = match base64::FromBase64::from_base64(content) {
            Ok(c) => c,
            Err(_) => {
                // XXX should return a 400 error
                return Promise::err(Error::failed("failed to convert from base64".into()));
            }
        };
        let mut grain_title: String = String::new();
        {
            let mut cursor = ::std::io::Cursor::new(decoded_content);
            let message = pry!(::capnp::serialize_packed::read_message(&mut cursor,
                                                                       Default::default()));
            let desc: powerbox_descriptor::Reader = pry!(message.get_root());
            for tag in pry!(desc.get_tags()).iter() {
                println!("tag {}", tag.get_id());
                let value: ui_view::powerbox_tag::Reader = pry!(tag.get_value().get_as());
                grain_title = pry!(value.get_title()).into();
                println!("grain title: {}", grain_title);

            }
        }

        // now let's save this thing into an actual uiview sturdyref
        let mut req = self.context.claim_request_request();
        let sandstorm_api = self.sandstorm_api.clone();
        req.get().set_request_token(&token[..]);
        let saved_ui_views = self.saved_ui_views.clone();
        let identity_id = self.identity_id.clone();
        let do_stuff = req.send().promise.then(move |response| {
            println!("restored!");
            let sealed_ui_view: ui_view::Client =
                pry!(pry!(response.get()).get_cap().get_as_capability());
            println!("got the cap!");

            let mut req = sandstorm_api.save_request();
            req.get().get_cap().set_as_capability(sealed_ui_view.client.hook);
            {
                let mut save_label = req.get().init_label();
                save_label.set_default_text("[save label chosen by collections app]");
            }
            req.send().promise.map(move |response| {
                let binary_token = try!(try!(response.get()).get_token());
                let token = base64::ToBase64::to_base64(binary_token, base64::URL_SAFE);

                try!(saved_ui_views.borrow_mut().insert(token.clone(), grain_title, identity_id));

                try!(SavedUiViewSet::retrieve_view_info(&saved_ui_views, token));
                Ok(())
            })
        });

        do_stuff.then_else(move |r| match r {
            Ok(()) => {
                let mut _content = results.get().init_content();
                Promise::ok(())
            }
            Err(e) => {
                let mut error = results.get().init_client_error();
                error.set_description_html(&format!("error: {:?}", e));
                Promise::ok(())
            }
        })
    }

    fn require_canonical_path(&self, path: &str) -> Result<(), Error> {
        // Require that the path doesn't contain "." or ".." or consecutive slashes, to prevent path
        // injection attacks.
        //
        // Note that such attacks wouldn't actually accomplish much since everything outside /var
        // is a read-only filesystem anyway, containing the app package contents which are non-secret.

        for (idx, component) in path.split_terminator("/").enumerate() {
            if component == "." || component == ".." || (component == "" && idx > 0) {
                return Err(Error::failed(format!("non-canonical path: {:?}", path)));
            }
        }
        Ok(())
    }

    fn read_file(&self,
                 filename: &str,
                 mut results: web_session::GetResults,
                 content_type: &str,
                 encoding: Option<&str>)
                 -> Promise<(), Error>
    {
        match ::std::fs::File::open(filename) {
            Ok(mut f) => {
                let size = pry!(f.metadata()).len();
                let mut content = results.get().init_content();
                content.set_status_code(web_session::response::SuccessCode::Ok);
                content.set_mime_type(content_type);
                encoding.map(|enc| content.set_encoding(enc));

                let mut body = content.init_body().init_bytes(size as u32);
                pry!(::std::io::copy(&mut f, &mut body));
                Promise::ok(())
            }
            Err(ref e) if e.kind() == ::std::io::ErrorKind::NotFound => {
                let mut error = results.get().init_client_error();
                error.set_status_code(web_session::response::ClientErrorCode::NotFound);
                Promise::ok(())
            }
            Err(e) => {
                Promise::err(e.into())
            }
        }
    }
}

pub struct UiView {
    timer: ::gjio::Timer,
    sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned>,
    saved_ui_views: Rc<RefCell<SavedUiViewSet>>,
}

impl UiView {
    fn new(timer: ::gjio::Timer,
           client: sandstorm_api::Client<::capnp::any_pointer::Owned>,
           saved_ui_views: Rc<RefCell<SavedUiViewSet>>) -> UiView
    {
        UiView {
            timer: timer,
            sandstorm_api: client,
            saved_ui_views: saved_ui_views,
        }
    }
}

impl ui_view::Server for UiView {
    fn get_view_info(&mut self,
                     _params: ui_view::GetViewInfoParams,
                     mut results: ui_view::GetViewInfoResults)
                     -> Promise<(), Error>
    {
        let mut view_info = results.get();

        // Define a "write" permission, and then define roles "editor" and "viewer" where only "editor"
        // has the "write" permission. This will allow people to share read-only.
        {
            let perms = view_info.borrow().init_permissions(1);
            let mut write = perms.get(0);
            write.set_name("write");
            write.init_title().set_default_text("write");
        }

        let mut roles = view_info.init_roles(2);
        {
            let mut editor = roles.borrow().get(0);
            editor.borrow().init_title().set_default_text("editor");
            editor.borrow().init_verb_phrase().set_default_text("can edit");
            editor.init_permissions(1).set(0, true);   // has "write" permission
        }
        {
            let mut viewer = roles.get(1);
            viewer.borrow().init_title().set_default_text("viewer");
            viewer.borrow().init_verb_phrase().set_default_text("can view");
            viewer.init_permissions(1).set(0, false);  // does not have "write" permission
        }
        Promise::ok(())
    }


    fn new_session(&mut self,
                   params: ui_view::NewSessionParams,
                   mut results: ui_view::NewSessionResults)
                   -> Promise<(), Error>
    {
        use ::capnp::traits::HasTypeId;
        let params = pry!(params.get());

        if params.get_session_type() != web_session::Client::type_id() {
            return Promise::err(Error::failed("unsupported session type".to_string()));
        }

        let session = pry!(WebSession::new(
            self.timer.clone(),
            pry!(params.get_user_info()),
            pry!(params.get_context()),
            pry!(params.get_session_params().get_as()),
            self.sandstorm_api.clone(),
            self.saved_ui_views.clone()));
        let client: web_session::Client =
            web_session::ToClient::new(session).from_server::<::capnp_rpc::Server>();

        // we need to do this dance to upcast.
        results.get().set_session(ui_session::Client { client : client.client});
        Promise::ok(())
    }
}

pub fn main() -> Result<(), Box<::std::error::Error>> {
    EventLoop::top_level(move |wait_scope| {
        let mut event_port = try!(::gjio::EventPort::new());
        let network = event_port.get_network();

        // sandstorm launches us with a connection on file descriptor 3
	    let stream = try!(unsafe { network.wrap_raw_socket_descriptor(3) });

        let (p, f) = Promise::and_fulfiller();
        let sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned> =
            ::capnp_rpc::new_promise_client(p);
        let saved_uiviews = try!(SavedUiViewSet::new("/var/sturdyrefs", sandstorm_api.clone()));


        let uiview = UiView::new(
            event_port.get_timer(),
            sandstorm_api,
            saved_uiviews);

        let client = ui_view::ToClient::new(uiview).from_server::<::capnp_rpc::Server>();
        let network =
            twoparty::VatNetwork::new(stream.clone(), stream,
                                      rpc_twoparty_capnp::Side::Client, Default::default());

	    let mut rpc_system = RpcSystem::new(Box::new(network), Some(client.client));
        let cap = rpc_system.bootstrap::<sandstorm_api::Client<::capnp::any_pointer::Owned>>(
            ::capnp_rpc::rpc_twoparty_capnp::Side::Server);
        f.fulfill(cap.client);
        Promise::never_done().wait(wait_scope, &mut event_port)
    })
}
