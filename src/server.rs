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

use multipoll::{Finisher, Poller, PollerHandle};
use capnp::Error;
use capnp::capability::Promise;
use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};
use base64::{self, Engine};

use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::cell::RefCell;
use std::rc::Rc;

use futures::{FutureExt, TryFutureExt};
use crate::collections_capnp::ui_view_metadata;
use crate::web_socket;
use crate::identity_map::IdentityMap;

use sandstorm::powerbox_capnp::powerbox_descriptor;
use sandstorm::identity_capnp::{user_info};
use sandstorm::grain_capnp::{session_context, ui_view, ui_session, sandstorm_api};
use sandstorm::util_capnp::{static_asset};
use sandstorm::web_session_capnp::{web_session};
use sandstorm::web_session_capnp::web_session::web_socket_stream;

pub struct WebSocketStream {
    id: u64,
    saved_ui_views: SavedUiViewSet,
}

impl Drop for WebSocketStream {
    fn drop(&mut self) {
        self.saved_ui_views.inner.borrow_mut().subscribers.remove(&self.id);
    }
}

impl WebSocketStream {
    fn new(id: u64,
           saved_ui_views: SavedUiViewSet)
           -> WebSocketStream
    {
        WebSocketStream {
            id: id,
            saved_ui_views: saved_ui_views,
        }
    }
}

impl web_socket::MessageHandler for WebSocketStream {
    fn handle_message(&mut self, message: web_socket::Message) -> Promise<(), Error> {
        // TODO: move PUTs and POSTs into websocket requests?
        match message {
            web_socket::Message::Text(_t) => {
            }
            web_socket::Message::Data(_d) => {
            }
        }
        Promise::ok(())
    }
}

#[derive(Clone)]
struct SavedUiViewData {
    title: String,
    date_added: u64,
    added_by: Option<String>,
}

// copied from rustc_serialize
fn json_escape_str(v: &str) -> String {
    let mut result: String = "\"".into();

    let mut start = 0;

    for (i, byte) in v.bytes().enumerate() {
        let escaped = match byte {
            b'\"' => "\\\"",
            b'\\' => "\\\\",
            b'\x00' => "\\u0000",
            b'\x01' => "\\u0001",
            b'\x02' => "\\u0002",
            b'\x03' => "\\u0003",
            b'\x04' => "\\u0004",
            b'\x05' => "\\u0005",
            b'\x06' => "\\u0006",
            b'\x07' => "\\u0007",
            b'\x08' => "\\b",
            b'\t' => "\\t",
            b'\n' => "\\n",
            b'\x0b' => "\\u000b",
            b'\x0c' => "\\f",
            b'\r' => "\\r",
            b'\x0e' => "\\u000e",
            b'\x0f' => "\\u000f",
            b'\x10' => "\\u0010",
            b'\x11' => "\\u0011",
            b'\x12' => "\\u0012",
            b'\x13' => "\\u0013",
            b'\x14' => "\\u0014",
            b'\x15' => "\\u0015",
            b'\x16' => "\\u0016",
            b'\x17' => "\\u0017",
            b'\x18' => "\\u0018",
            b'\x19' => "\\u0019",
            b'\x1a' => "\\u001a",
            b'\x1b' => "\\u001b",
            b'\x1c' => "\\u001c",
            b'\x1d' => "\\u001d",
            b'\x1e' => "\\u001e",
            b'\x1f' => "\\u001f",
            b'\x7f' => "\\u007f",
            _ => { continue; }
        };

        if start < i {
            result.push_str(&v[start..i]);
        }

        result.push_str(escaped);

        start = i + 1;
    }

    if start != v.len() {
        result.push_str(&v[start..]);
    }

    result.push_str("\"");
    result
}

#[test]
fn test_json_escape_string() {
    assert_eq!(json_escape_str("hello"), "\"hello\"");
    assert_eq!(json_escape_str("he\"\"llo"), "\"he\\\"\\\"llo\"");
}

fn optional_string_to_json(optional_string: &Option<String>) -> String {
    match optional_string {
        &None => "null".into(),
        &Some(ref s) => format!("{}", json_escape_str(s)),
    }
}

impl SavedUiViewData {
    fn to_json(&self) -> String {
        format!("{{\"title\":{},\"dateAdded\": \"{}\",\"addedBy\":{}}}",
                json_escape_str(&self.title),
                self.date_added,
                optional_string_to_json(&self.added_by))
    }
}

#[derive(Clone, Debug)]
struct ViewInfoData {
    app_title: String,
    grain_icon_url: String,
}

impl ViewInfoData {
    fn to_json(&self) -> String {
        format!("{{\"appTitle\":{},\"grainIconUrl\":\"{}\"}}",
                json_escape_str(&self.app_title),
                self.grain_icon_url)
    }
}

#[derive(Clone, Debug)]
struct ProfileData {
    display_name: String,
    picture_url: String,
}

impl ProfileData {
    fn to_json(&self) -> String {
        format!(
            "{{\"pictureUrl\":{}, \"displayName\":{}}}",
            json_escape_str(&self.picture_url),
            json_escape_str(&self.display_name))
    }
}

#[derive(Clone)]
enum Action {
    Insert { token: String, data: SavedUiViewData },
    Remove { token: String },
    ViewInfo { token: String, data: Result<ViewInfoData, Error> },
    CanWrite(bool),
    UserId(Option<String>),
    Description(String),
    User { id: String, data: ProfileData },
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
            &Action::ViewInfo { ref token, data: Ok(ref data) } => {
                format!("{{\"viewInfo\":{{\"token\":\"{}\",\"data\":{} }} }}",
                        token, data.to_json())
            }
            &Action::ViewInfo { ref token, data: Err(ref e) } => {
                format!("{{\"viewInfo\":{{\"token\":\"{}\",\"failed\": {} }} }}",
                        token,
                        json_escape_str(&format!("{}", e)))
            }

            &Action::CanWrite(b) => {
                format!("{{\"canWrite\":{}}}", b)
            }
            &Action::UserId(ref s) => {
                format!("{{\"userId\":{}}}", optional_string_to_json(s))
            }
            &Action::Description(ref s) => {
                format!("{{\"description\":{}}}", json_escape_str(s))
            }
            &Action::User { ref id, ref data } => {
                format!(
                    "{{\"user\":{{\"id\":{}, \"data\":{} }}}}",
                    json_escape_str(id), data.to_json())
            }
        }
    }
}

fn url_of_static_asset(asset: static_asset::Client) -> Promise<String, Error> {
    Promise::from_future(asset.get_url_request().send().promise.map(
        move |r| match r {
            Ok(response) => {
                let result = response.get()?;
                let protocol = match result.get_protocol()? {
                    static_asset::Protocol::Https => "https".to_string(),
                    static_asset::Protocol::Http => "http".to_string(),
                };

                Ok(format!("{}://{}", protocol, result.get_host_path()?.to_str()?))
            }
            Err(e) => Err(e),
        }
    ))
}

struct Reaper;

impl Finisher<Error> for Reaper {
    fn task_failed(&mut self, error: Error) {
        // TODO better message.
        println!("task failed: {}", error);
    }
}

struct SavedUiViewSetInner {
    tmp_dir: ::std::path::PathBuf,
    sturdyref_dir: ::std::path::PathBuf,

    /// Invariant: Every entry in this map has been persisted to the filesystem and has sent
    /// out Action::Insert messages to each subscriber.
    views: HashMap<String, SavedUiViewData>,

    view_infos: HashMap<String, Result<ViewInfoData, Error>>,
    next_id: u64,
    subscribers: HashMap<u64, web_socket_stream::Client>,
    tasks: PollerHandle<Error>,
    description: String,
    sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned>,
    identity_map: IdentityMap,

}

impl SavedUiViewSetInner {
    fn get_saved_data<'a>(&'a self, token: &'a String) -> Option<&'a SavedUiViewData> {
        self.views.get(token)
    }
}

#[derive(Clone)]
pub struct SavedUiViewSet {
    inner: Rc<RefCell<SavedUiViewSetInner>>,
}

impl SavedUiViewSet {
    pub fn new<P1, P2>(tmp_dir: P1,
                       sturdyref_dir: P2,
                       sandstorm_api: &sandstorm_api::Client<::capnp::any_pointer::Owned>,
                       identity_map: IdentityMap,
    )
                  -> ::capnp::Result<SavedUiViewSet>
        where P1: AsRef<::std::path::Path>,
              P2: AsRef<::std::path::Path>
    {
        let description = match ::std::fs::File::open("/var/description") {
            Ok(mut f) => {
                use std::io::Read;
                let mut result = String::new();
                f.read_to_string(&mut result)?;
                result
            }
            Err(ref e) if e.kind() == ::std::io::ErrorKind::NotFound => {
                use std::io::Write;
                let mut f = ::std::fs::File::create("/var/description")?;
                let result = "";
                f.write_all(result.as_bytes())?;
                result.into()
            }
            Err(e) => {
                return Err(e.into());
            }
        };

        let (tx, poller) = Poller::new(Box::new(Reaper));
        tokio::task::spawn_local(poller.map_err(|_|()));

        let result = SavedUiViewSet {
            inner: Rc::new(RefCell::new(SavedUiViewSetInner {
                tmp_dir: tmp_dir.as_ref().to_path_buf(),
                sturdyref_dir: sturdyref_dir.as_ref().to_path_buf(),
                views: HashMap::new(),
                view_infos: HashMap::new(),
                next_id: 0,
                subscribers: HashMap::new(),
                tasks: tx,
                description: description,
                sandstorm_api: sandstorm_api.clone(),
                identity_map: identity_map,
            })),
        };

        // create sturdyref directory if it does not yet exist
        ::std::fs::create_dir_all(&sturdyref_dir)?;

        // clear and create tmp directory
        match ::std::fs::remove_dir_all(&tmp_dir) {
            Ok(()) => (),
            Err(ref e) if e.kind() == ::std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
        ::std::fs::create_dir_all(&tmp_dir)?;

        for token_file in ::std::fs::read_dir(&sturdyref_dir)? {
            let dir_entry = token_file?;
            let token: String = match dir_entry.file_name().to_str() {
                None => {
                    println!("malformed token: {:?}", dir_entry.file_name());
                    continue
                }
                Some(s) => s.into(),
            };

            if token.ends_with(".uploading") {
                // At one point, these temporary files got uploading directly into this directory.
                ::std::fs::remove_file(dir_entry.path())?;
            } else {
                let mut reader = ::std::fs::File::open(dir_entry.path())?;
                let message = ::capnp::serialize::read_message(&mut reader,
                                                               Default::default())?;
                let metadata: ui_view_metadata::Reader = message.get_root()?;

                let added_by = if metadata.has_added_by() {
                    Some(metadata.get_added_by()?.to_string()?)
                } else {
                    None
                };

                let entry = SavedUiViewData {
                    title: metadata.get_title()?.to_string()?,
                    date_added: metadata.get_date_added(),
                    added_by: added_by,
                };

                result.inner.borrow_mut().views.insert(token.clone(), entry);

                result.retrieve_view_info(token)?;
            }
        }

        Ok(result)
    }

    fn retrieve_view_info(&self,
                          token: String) -> ::capnp::Result<()> {
        // SandstormApi.restore, then call getViewInfo,
        // then call get_url() on the grain static asset.

        let self1 = self.clone();
        let binary_token = match base64::engine::general_purpose::URL_SAFE.decode(&token[..]) {
            Ok(b) => b,
            Err(e) => return Err(Error::failed(format!("{}", e))),
        };

        let mut req = self.inner.borrow().sandstorm_api.restore_request();
        req.get().set_token(&binary_token);
        let task = req.send().promise.and_then(move |response| {
            let view: ui_view::Client =
                pry!(pry!(response.get()).get_cap().get_as_capability());
            Promise::from_future(view.get_view_info_request().send().promise.and_then(move |response| {
                let view_info = pry!(response.get());
                let app_title = pry!(pry!(pry!(view_info.get_app_title()).get_default_text()).to_string());
                Promise::from_future(url_of_static_asset(pry!(view_info.get_grain_icon())).map_ok(move |url| {
                    ViewInfoData {
                        app_title: app_title,
                        grain_icon_url: url,
                    }
                }))
            }))
        }).map(move |result| {
            self1.inner.borrow_mut().view_infos.insert(token.clone(), result.clone());
            self1.send_action_to_subscribers(Action::ViewInfo {
                token: token,
                data: result,
            });

            Ok(())
        });

        self.inner.borrow_mut().tasks.add(task);
        Ok(())
    }

    fn get_user_profile(&self,
                        identity_id: &str) -> Promise<ProfileData, Error> {
        Promise::from_future(self.inner.borrow_mut().identity_map.get_by_text(identity_id).and_then(move |identity| {
            identity.get_profile_request().send().promise
        }).and_then(move |response| {
            let profile = pry!(pry!(response.get()).get_profile());
            let display_name = pry!(pry!(pry!(profile.get_display_name()).get_default_text()).to_string());
            Promise::from_future(url_of_static_asset(pry!(profile.get_picture())).map_ok(move |url| {
                ProfileData { display_name: display_name, picture_url: url }
            }))
        }))
    }

    fn update_description(&self, description: &[u8]) -> ::capnp::Result<()> {
        use std::io::Write;

        let desc_string: String = match ::std::str::from_utf8(description) {
            Err(e) => return Err(::capnp::Error::failed(format!("{}", e))),
            Ok(d) => d.into(),
        };

        let temp_path = format!("/var/description.uploading");
        ::std::fs::File::create(&temp_path)?.write_all(description)?;
        ::std::fs::rename(temp_path, "/var/description")?;

        self.inner.borrow_mut().description = desc_string.clone();
        self.send_action_to_subscribers(Action::Description(desc_string));
        Ok(())
    }

    fn insert(&mut self,
              token: String,
              title: String,
              added_by: Option<String>) -> ::capnp::Result<()> {
        let dur = ::std::time::SystemTime::now().duration_since(::std::time::UNIX_EPOCH)
            .map_err(|e| Error::failed(format!("{}", e)))?;
        let date_added = dur.as_secs() * 1000 + (dur.subsec_nanos() / 1000000) as u64;

        let mut token_path = ::std::path::PathBuf::new();
        token_path.push(self.inner.borrow().sturdyref_dir.clone());
        token_path.push(token.clone());

        let mut temp_path = ::std::path::PathBuf::new();
        temp_path.push(self.inner.borrow().tmp_dir.clone());
        temp_path.push(format!("{}.uploading", token));

        let mut writer = ::std::fs::File::create(&temp_path)?;

        let mut message = ::capnp::message::Builder::new_default();
        {
            let mut metadata: ui_view_metadata::Builder = message.init_root();
            metadata.set_title(&title);
            metadata.set_date_added(date_added);
            match added_by {
                Some(ref s) => metadata.set_added_by(s),
                None => (),
            }
        }

        ::capnp::serialize::write_message(&mut writer, &message)?;
        ::std::fs::rename(temp_path, token_path)?;
        writer.sync_all()?;

        if !self.inner.borrow().subscribers.is_empty() {
            if let Some(ref id) = added_by {
                let self1 = self.clone();
                let identity_id: String = id.to_string();
                let task = self.get_user_profile(&identity_id).map_ok(move |profile_data| {
                    self1.send_action_to_subscribers(
                        Action::User { id: identity_id, data: profile_data });
                });
                self.inner.borrow_mut().tasks.add(task);
            }
        }

        let entry = SavedUiViewData {
            title: title,
            date_added: date_added,
            added_by: added_by,
        };

        self.send_action_to_subscribers(Action::Insert {
            token: token.clone(),
            data: entry.clone(),
        });
        self.inner.borrow_mut().views.insert(token, entry);

        Ok(())
    }

    fn send_action_to_subscribers(&self, action: Action) {
        let json_string = action.to_json();
        let &mut SavedUiViewSetInner { ref subscribers, ref mut tasks, ..} =
            &mut *self.inner.borrow_mut();
        for (_, sub) in &*subscribers {
            let mut req = sub.send_bytes_request();
            web_socket::encode_text_message(req.get(), &json_string);
            tasks.add(req.send().promise.map_ok(|_| ()));
        }
    }

    fn remove(&mut self, token: &str) -> Result<(), Error> {
        let mut path = self.inner.borrow().sturdyref_dir.clone();
        path.push(token);
        if let Err(e) = ::std::fs::remove_file(path) {
            if e.kind() != ::std::io::ErrorKind::NotFound {
                return Err(e.into())
            }
        }

        self.send_action_to_subscribers(Action::Remove { token: token.into() });
        self.inner.borrow_mut().views.remove(token);
        Ok(())
    }

    fn new_subscribed_websocket(&self,
                                client_stream: web_socket_stream::Client,
                                can_write: bool,
                                user_id: Option<String>)
                                 -> web_socket_stream::Client
    {
        fn send_action(task: Promise<(), Error>,
                       client_stream: &web_socket_stream::Client,
                       action: Action) -> Promise<(), Error> {
            let json_string = action.to_json();
            let mut req = client_stream.send_bytes_request();
            web_socket::encode_text_message(req.get(), &json_string);
            let promise = req.send().promise.map_ok(|_| ());
            Promise::from_future(task.and_then(|_| promise))
        }

        let id = self.inner.borrow().next_id;
        self.inner.borrow_mut().next_id = id + 1;

        self.inner.borrow_mut().subscribers.insert(id, client_stream.clone());

        let mut task = Promise::ok(());

        task = send_action(task, &client_stream, Action::CanWrite(can_write));
        task = send_action(task, &client_stream, Action::UserId(user_id));
        task = send_action(task, &client_stream,
                           Action::Description(self.inner.borrow().description.clone()));

        let mut added_by_identities: HashSet<String> = HashSet::new();

        for (t, v) in &self.inner.borrow().views {
            if let &Some(ref id) = &v.added_by {
                added_by_identities.insert(id.clone());
            }

            task = send_action(
                task, &client_stream,
                Action::Insert {
                    token: t.clone(),
                    data: v.clone()
                }
            );
        }

        for (t, vi) in &self.inner.borrow().view_infos {
            task = send_action(
                task, &client_stream,
                Action::ViewInfo {
                    token: t.clone(),
                    data: vi.clone(),
                }
            );
        }

        self.inner.borrow_mut().tasks.add(task);

        for ref text_id in &added_by_identities {
            let id = text_id.to_string();
            let client_stream1 = client_stream.clone();

            let task = self.get_user_profile(text_id).and_then(move |profile_data| {
                let action = Action::User { id: id, data: profile_data };
                let json_string = action.to_json();
                let mut req = client_stream1.send_bytes_request();
                web_socket::encode_text_message(req.get(), &json_string);
                req.send().promise.map_ok(|_| ())
            });

            self.inner.borrow_mut().tasks.add(task);
        }

        capnp_rpc::new_client(
            web_socket::Adapter::new(
                WebSocketStream::new(id, self.clone()),
                client_stream,
                self.inner.borrow().tasks.clone()))
    }
}

const ADD_GRAIN_ACTIVITY_INDEX: u16 = 0;
const REMOVE_GRAIN_ACTIVITY_INDEX: u16 = 1;
const EDIT_DESCRIPTION_ACTIVITY_INDEX: u16 = 2;

pub struct WebSession {
    can_write: bool,
    sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned>,
    context: session_context::Client,
    saved_ui_views: SavedUiViewSet,
    identity_id: Option<String>,
}

impl WebSession {
    pub fn new(user_info: user_info::Reader,
               context: session_context::Client,
               _params: web_session::params::Reader,
               sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned>,
               saved_ui_views: SavedUiViewSet)
               -> ::capnp::Result<WebSession>
    {
        // Permission #0 is "write". Check if bit 0 in the PermissionSet is set.
        let permissions = user_info.get_permissions()?;
        let can_write = permissions.len() > 0 && permissions.get(0);
        let identity_id = if user_info.has_identity_id() {
            Some(::hex::encode(user_info.get_identity_id()?))
        } else {
            None
        };

        Ok(WebSession {
            can_write: can_write,
            sandstorm_api: sandstorm_api,
            context: context,
            saved_ui_views: saved_ui_views,
            identity_id: identity_id,
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
    async fn get(&self,
           params: web_session::GetParams,
           mut results: web_session::GetResults)
	-> Result<(), Error>
    {
        // HTTP GET request.
        let path = params.get()?.get_path()?.to_str()?;
        self.require_canonical_path(path)?;

        if path == "" {
            let text = "<!DOCTYPE html>\
                       <html><head>\
                       <link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\">\
                       <script type=\"text/javascript\" src=\"script.js\" async></script>
                       </head><body><div id=\"main\"></div></body></html>";
            let mut content = results.get().init_content();
            content.set_mime_type("text/html; charset=UTF-8");
            content.init_body().set_bytes(text.as_bytes());
            Ok(())
        } else if path == "script.js" {
            self.read_file("/script.js.gz", results, "text/javascript; charset=UTF-8", Some("gzip"))
        } else if path == "style.css" {
            self.read_file("/style.css.gz", results, "text/css; charset=UTF-8", Some("gzip"))
        } else {
            let mut error = results.get().init_client_error();
            error.set_status_code(web_session::response::ClientErrorCode::NotFound);
            Ok(())
        }
    }

    async fn post(&self,
            params: web_session::PostParams,
            mut results: web_session::PostResults)
            -> Result<(), Error>
    {
        let path = {
            let path = params.get()?.get_path()?.to_str()?;
            self.require_canonical_path(path)?;
            path.to_string()
        };

        if path.starts_with("token/") {
            self.receive_request_token(path[6..].to_string(), params, results).await
        } else if path.starts_with("offer/") {
            let token = path[6..].to_string();
            let title = match self.saved_ui_views.inner.borrow().get_saved_data(&token) {
                None => {
                    let mut error = results.get().init_client_error();
                    error.set_status_code(web_session::response::ClientErrorCode::NotFound);
                    return Ok(())
                }
                Some(saved_ui_view) => saved_ui_view.title.to_string(),
            };

            self.offer_ui_view(token, title, params, results).await
        } else if path.starts_with("refresh/") {
            let token = path[8..].to_string();
            match SavedUiViewSet::retrieve_view_info(&self.saved_ui_views, token) {
                Ok(()) => {
                    results.get().init_no_content();
                }
                Err(e) => {
                    fill_in_client_error(results, e);
                }
            }
            Ok(())
        } else {
            let mut error = results.get().init_client_error();
            error.set_status_code(web_session::response::ClientErrorCode::NotFound);
            Ok(())
        }
    }

    async fn put(&self,
           params: web_session::PutParams,
           mut results: web_session::PutResults)
	-> Result<(), Error>
    {
        // HTTP PUT request.

        let params = params.get()?;
        let path = params.get_path()?.to_str()?;
        self.require_canonical_path(path)?;

        if !self.can_write {
            results.get().init_client_error()
                .set_status_code(web_session::response::ClientErrorCode::Forbidden);
            Ok(())
        } else if path == "description" {
            let content = params.get_content()?.get_content()?;
            self.saved_ui_views.update_description(content)?;
            let mut req = self.context.activity_request();
            req.get().init_event().set_type(EDIT_DESCRIPTION_ACTIVITY_INDEX);
            req.send().promise.await?;
            results.get().init_no_content();
            Ok(())
        } else {
            results.get().init_client_error()
                .set_status_code(web_session::response::ClientErrorCode::Forbidden);
            Ok(())
        }
    }

    async fn delete(&self,
                    params: web_session::DeleteParams,
                    mut results: web_session::DeleteResults)
	-> Result<(), Error>
    {
        // HTTP DELETE request.

        let path = params.get()?.get_path()?.to_str()?;
        self.require_canonical_path(path)?;

        if !path.starts_with("sturdyref/") {
            return Err(Error::failed("DELETE only supported under sturdyref/".to_string()));
        }

        if !self.can_write {
            results.get().init_client_error()
                .set_status_code(web_session::response::ClientErrorCode::Forbidden);
            Ok(())
        } else {
            let token_string = path[10..].to_string();
            let binary_token = match base64::engine::general_purpose::URL_SAFE.decode(&token_string[..]) {
                Ok(b) => b,
                Err(e) => {
                    results.get().init_client_error().set_description_html(&format!("{}", e));
                    return Ok(())
                }
            };

            let mut saved_ui_views = self.saved_ui_views.clone();
            let context = self.context.clone();
            let mut req = self.sandstorm_api.drop_request();
            req.get().set_token(&binary_token);
            req.send().promise.await?;
            saved_ui_views.remove(&token_string)?;
            let mut req = context.activity_request();
            req.get().init_event().set_type(REMOVE_GRAIN_ACTIVITY_INDEX);
            req.send().promise.await?;
            results.get().init_no_content();
            Ok(())
        }
    }

    async fn open_web_socket(&self,
                             params: web_session::OpenWebSocketParams,
                             mut results: web_session::OpenWebSocketResults)
                             -> Result<(), Error>
    {
        let client_stream = params.get()?.get_client_stream()?;

        results.get().set_server_stream(
            self.saved_ui_views.new_subscribed_websocket(
                client_stream,
                self.can_write,
                self.identity_id.clone()));

        Ok(())
    }
}

fn fill_in_client_error(mut results: web_session::PostResults, e: Error)
{
    let mut client_error = results.get().init_client_error();
    client_error.set_description_html(&format!("{}", e));
}

impl WebSession {
    fn offer_ui_view(&self,
                     text_token: String,
                     title: String,
                     _params: web_session::PostParams,
                     mut results: web_session::PostResults)
                     -> Promise<(), Error>
    {
        let token = match base64::engine::general_purpose::URL_SAFE.decode(&text_token[..]) {
            Ok(b) => b,
            Err(e) => return Promise::err(Error::failed(format!("{}", e))),
        };

        let session_context = self.context.clone();
        let set = self.saved_ui_views.clone();
        let mut req = self.sandstorm_api.restore_request();
        req.get().set_token(&token);
        Promise::from_future(req.send().promise.then(move |response| match response {
            Ok(v) => {
                let sealed_ui_view: ui_view::Client =
                    pry!(pry!(v.get()).get_cap().get_as_capability());
                let mut req = session_context.offer_request();
                req.get().get_cap().set_as_capability(sealed_ui_view.client.hook);
                {
                    use capnp::traits::HasTypeId;
                    let tags = req.get().init_descriptor().init_tags(1);
                    let mut tag = tags.get(0);
                    tag.set_id(ui_view::Client::TYPE_ID);
                    let mut value: ui_view::powerbox_tag::Builder = tag.get_value().init_as();
                    value.set_title(&title);
                }

                Promise::from_future(req.send().promise.map_ok(|_| ()))
            }
            Err(e) => {
                set.inner.borrow_mut().view_infos.insert(text_token.clone(), Err(e.clone()));
                set.send_action_to_subscribers(Action::ViewInfo {
                    token: text_token,
                    data: Err(e),
                });
                Promise::ok(())
            }
        }).then(move |r| match r {
            Ok(_) => {
                results.get().init_no_content();
                Promise::ok(())
            }
            Err(e) => {
                fill_in_client_error(results, e);
                Promise::ok(())
            }
        }))
    }

    fn read_powerbox_tag(&self, decoded_content: Vec<u8>) -> ::capnp::Result<String>
    {
        let mut cursor = ::std::io::Cursor::new(decoded_content);
        let message = ::capnp::serialize_packed::read_message(&mut cursor,
                                                              Default::default())?;
        let desc: powerbox_descriptor::Reader = message.get_root()?;
        let tags = desc.get_tags()?;
        if tags.len() == 0 {
            Err(Error::failed("no powerbox tag".into()))
        } else {
            let value: ui_view::powerbox_tag::Reader = tags.get(0).get_value().get_as()?;
            Ok(value.get_title()?.to_string()?)
        }
    }

    fn receive_request_token(&self,
                             token: String,
                             params: web_session::PostParams,
                             mut results: web_session::PostResults)
                             -> Promise<(), Error>
    {
        let content = pry!(pry!(pry!(params.get()).get_content()).get_content());

        let decoded_content = match base64::engine::general_purpose::URL_SAFE.decode(content) {
            Ok(c) => c,
            Err(_) => {
                fill_in_client_error(results, Error::failed("failed to convert from base64".into()));
                return Promise::ok(())
            }
        };
        let grain_title: String = match self.read_powerbox_tag(decoded_content) {
            Ok(t) => t,
            Err(e) => {
                fill_in_client_error(results, e);
                return Promise::ok(());
            }
        };

        // now let's save this thing into an actual uiview sturdyref
        let mut req = self.context.claim_request_request();
        let sandstorm_api = self.sandstorm_api.clone();
        req.get().set_request_token(&token);
        let mut saved_ui_views = self.saved_ui_views.clone();
        let identity_id = self.identity_id.clone();

        let do_stuff = req.send().promise.and_then(move |response| {
            let sealed_ui_view: ui_view::Client =
                pry!(pry!(response.get()).get_cap().get_as_capability());
            let mut req = sandstorm_api.save_request();
            req.get().get_cap().set_as_capability(sealed_ui_view.client.hook);
            {
                let mut save_label = req.get().init_label();
                save_label.set_default_text(&format!("grain with title: {}", grain_title));
            }
            Promise::from_future(req.send().promise.map(move |r| {
                let response = r?;
                let binary_token = response.get()?.get_token()?;
                let token = base64::engine::general_purpose::URL_SAFE.encode(binary_token);

                saved_ui_views.insert(token.clone(), grain_title, identity_id)?;

                SavedUiViewSet::retrieve_view_info(&saved_ui_views, token)?;
                Ok(())
            }))
        });

        let context = self.context.clone();
        Promise::from_future(do_stuff.then(move |r| match r {
            Ok(()) => {
                let mut req = context.activity_request();
                req.get().init_event().set_type(ADD_GRAIN_ACTIVITY_INDEX);
                Promise::from_future(req.send().promise.and_then(move |_| {
                    let mut _content = results.get().init_content();
                    Promise::ok(())
                }))
            }
            Err(e) => {
                let mut error = results.get().init_client_error();
                error.set_description_html(&format!("error: {:?}", e));
                Promise::ok(())
            }
        }))
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
                 -> Result<(), Error>
    {
        match ::std::fs::File::open(filename) {
            Ok(mut f) => {
                let size = f.metadata()?.len();
                let mut content = results.get().init_content();
                content.set_status_code(web_session::response::SuccessCode::Ok);
                content.set_mime_type(content_type);
                encoding.map(|enc| content.set_encoding(enc));

                let mut body = content.init_body().init_bytes(size as u32);
                ::std::io::copy(&mut f, &mut body)?;
                Ok(())
            }
            Err(ref e) if e.kind() == ::std::io::ErrorKind::NotFound => {
                let mut error = results.get().init_client_error();
                error.set_status_code(web_session::response::ClientErrorCode::NotFound);
                Ok(())
            }
            Err(e) => {
                Err(e.into())
            }
        }
    }
}

pub struct UiView {
    sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned>,
    saved_ui_views: SavedUiViewSet,
}

impl UiView {
    fn new(client: sandstorm_api::Client<::capnp::any_pointer::Owned>,
           saved_ui_views: SavedUiViewSet)
           -> UiView
    {
        UiView {
            sandstorm_api: client,
            saved_ui_views: saved_ui_views,
        }
    }
}

impl ui_view::Server for UiView {
    async fn get_view_info(&self,
                     _params: ui_view::GetViewInfoParams,
                     mut results: ui_view::GetViewInfoResults)
                     -> Result<(), Error>
    {
        let mut view_info = results.get();

        // Define a "write" permission, and then define roles "editor" and "viewer" where only
        // "editor" has the "write" permission. This will allow people to share read-only.
        {
            let perms = view_info.reborrow().init_permissions(1);
            let mut write = perms.get(0);
            write.set_name("write");
            write.init_title().set_default_text("write");
        }

        {
            let mut roles = view_info.reborrow().init_roles(2);
            {
                let mut editor = roles.reborrow().get(0);
                editor.reborrow().init_title().set_default_text("editor");
                editor.reborrow().init_verb_phrase().set_default_text("can edit");
                editor.init_permissions(1).set(0, true);   // has "write" permission
            }
            {
                let mut viewer = roles.get(1);
                viewer.set_default(true);
                viewer.reborrow().init_title().set_default_text("viewer");
                viewer.reborrow().init_verb_phrase().set_default_text("can view");
                viewer.init_permissions(1).set(0, false);  // does not have "write" permission
            }
        }

        {
            let mut event_types = view_info.init_event_types(3);
            {
                let mut added = event_types.reborrow().get(ADD_GRAIN_ACTIVITY_INDEX as u32);
                added.set_name("add");
                added.reborrow().init_verb_phrase().set_default_text("added grain");
            }
            {
                let mut removed = event_types.reborrow().get(REMOVE_GRAIN_ACTIVITY_INDEX as u32);
                removed.set_name("remove");
                removed.reborrow().init_verb_phrase().set_default_text("removed grain");
            }
            {
                let mut removed = event_types.reborrow().get(EDIT_DESCRIPTION_ACTIVITY_INDEX as u32);
                removed.set_name("description");
                removed.reborrow().init_verb_phrase().set_default_text("edited description");
            }
        }

        Ok(())
    }


    async fn new_session(&self,
                   params: ui_view::NewSessionParams,
                   mut results: ui_view::NewSessionResults)
                   -> Result<(), Error>
    {
        use ::capnp::traits::HasTypeId;
        let params = params.get()?;

        if params.get_session_type() != web_session::Client::TYPE_ID {
            return Err(Error::failed("unsupported session type".to_string()));
        }

        let user_info = params.get_user_info()?;

        let session = WebSession::new(
            user_info.clone(),
            params.get_context()?,
            params.get_session_params().get_as()?,
            self.sandstorm_api.clone(),
            self.saved_ui_views.clone())?;
        let client: web_session::Client = capnp_rpc::new_client(session);

        // We need to do this silly dance to upcast.
        results.get().set_session(ui_session::Client { client : client.client});

        if user_info.has_identity_id() {
            let identity = user_info.get_identity()?;

            // TODO(cleanup)
            self.saved_ui_views.inner.borrow_mut().identity_map.put(user_info.get_identity_id()?, identity)?;
        }

        Ok(())
    }
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ::std::os::unix::io::{FromRawFd};
    use futures::io::AsyncReadExt;

    let mut rt = tokio::runtime::Runtime::new().unwrap();
    let local = tokio::task::LocalSet::new();

    local.block_on(&mut rt, async move {
        let stream: ::std::os::unix::net::UnixStream = unsafe { FromRawFd::from_raw_fd(3) };
        let stream = tokio::net::UnixStream::from_std(stream)?;
        let (read_half, write_half) =
            tokio_util::compat::Tokio02AsyncReadCompatExt::compat(stream).split();

        let network =
            Box::new(twoparty::VatNetwork::new(read_half, write_half,
                                               rpc_twoparty_capnp::Side::Client,
                                               Default::default()));

        let (tx, rx) = futures::channel::oneshot::channel();
        let sandstorm_api: sandstorm_api::Client<::capnp::any_pointer::Owned> =
            ::capnp_rpc::new_future_client(rx.map_err(|_e| capnp::Error::failed(format!("oneshot was canceled"))));

        let identity_map = IdentityMap::new(
            "/var/identities",
            "/var/trash",
            &sandstorm_api)?;
        let saved_uiviews = SavedUiViewSet::new(
            "/var/tmp",
            "/var/sturdyrefs",
            &sandstorm_api,
            identity_map)?;

        let uiview = UiView::new(
            sandstorm_api,
            saved_uiviews);

        let client: ui_view::Client = capnp_rpc::new_client(uiview);
        let mut rpc_system = RpcSystem::new(network, Some(client.client));

        let _ = tx.send(rpc_system.bootstrap::<sandstorm_api::Client<::capnp::any_pointer::Owned>>(
            ::capnp_rpc::rpc_twoparty_capnp::Side::Server));

        Ok::<_,  Box<dyn std::error::Error>>(rpc_system.await?)
    })?;
    Ok(())
}
