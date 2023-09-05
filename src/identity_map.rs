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

use capnp::capability::Promise;
use capnp::Error;
use futures::{FutureExt, TryFutureExt};
use url::percent_encoding;
use std::cell::RefCell;
use std::rc::Rc;

use sandstorm::identity_capnp::{identity};
use sandstorm::grain_capnp::{sandstorm_api};

fn read_sturdyref_symlink(pointed_to: ::std::path::PathBuf) -> Result<Vec<u8>, Error>
{
    let encoded_sturdyref = match pointed_to.to_str() {
        Some(s) => s.to_string(),
        None =>
            return Err(Error::failed(
                format!("invalid sturdyref symlink {:?}", pointed_to))),
    };

    let mut sturdyref: Vec<u8> = encoded_sturdyref.as_bytes().into();
    match percent_encoding::percent_decode(encoded_sturdyref.as_bytes()).if_any() {
        Some(s) => { sturdyref = s }
        None => (),
    }
    Ok(sturdyref)
}


struct Reaper;

impl ::multipoll::Finisher<Error> for Reaper {
    fn task_failed(&mut self, error: Error) {
        println!("IdentityMap task failed: {}", error);
    }
}

struct IdentityMapInner {
    directory: ::std::path::PathBuf,
    trash_directory: ::std::path::PathBuf,
    api: sandstorm_api::Client<::capnp::any_pointer::Owned>,
    tasks: ::multipoll::PollerHandle<Error>,
}

impl IdentityMapInner {
    fn read_from_disk(inner: &Rc<RefCell<IdentityMapInner>>,
                     truncated_text_id: &str) -> Promise<identity::Client, Error>
    {
        let mut symlink = inner.borrow().directory.clone();
        symlink.push(truncated_text_id);

        let pointed_to = pry!(::std::fs::read_link(symlink));
        let sturdyref = pry!(read_sturdyref_symlink(pointed_to));

        let mut req = inner.borrow().api.restore_request();
        req.get().set_token(&sturdyref[..]);

        Promise::from_future(req.send().promise.map(move |r| match r {
            Ok(response) => response.get()?.get_cap().get_as_capability(),
            Err(e) => Err(e),
        }))
    }

   fn save_to_disk(inner: &Rc<RefCell<IdentityMapInner>>,
                   truncated_text_id: &str,
                   identity: identity::Client) {
       let mut req = inner.borrow().api.save_request();
       req.get().init_cap().set_as_capability(identity.client.hook);
       req.get().init_label().set_default_text("user identity".into());
       let mut symlink = inner.borrow().directory.clone();
       symlink.push(&truncated_text_id);

       let inner1 = inner.clone();
       inner.borrow_mut().tasks.add(req.send().promise.map(move |r| match r {
           Ok(result) => {
               // We save the token as a symlink, which ext4 can store (up to 60 bytes)
               // directly in the inode, avoiding the need to allocate a block.
               //
               // Tokens are primarily text but can contain arbitrary bytes.
               // We percent-encode to be safe and to keep the length of the encoded
               // token under 60 bytes in the common case.

               let token = result.get()?.get_token()?;
               let encoded_token = percent_encoding::percent_encode(
                   token,
                   percent_encoding::DEFAULT_ENCODE_SET
               ).collect::<String>();

               IdentityMapInner::drop_identity(&inner1, &symlink)?;

               ::std::os::unix::fs::symlink(encoded_token, symlink)?;
               // TODO fsync?

               Ok(())
           }
           Err(e) => Err(e),
       }));
   }

    fn drop_identity<P>(inner: &Rc<RefCell<IdentityMapInner>>,
                        symlink: &P) -> Result<(), Error>
        where P: AsRef<::std::path::Path>
    {
        match ::std::fs::read_link(symlink) {
            Ok(pointed_to) => {
                // symlink exists!
                let mut trash_file = inner.borrow().trash_directory.clone();
                trash_file.push(&pointed_to);
                ::std::fs::rename(symlink, &trash_file)?;

                let mut req = inner.borrow().api.drop_request();
                let sturdyref = read_sturdyref_symlink(pointed_to)?;
                req.get().set_token(&sturdyref[..]);
                inner.borrow_mut().tasks.add(req.send().promise.map(move |r| match r {
                    Ok(_) => {
                        ::std::fs::remove_file(trash_file)?;
                        // TODO fsync?
                        Ok(())
                    }
                    Err(e) => Err(e),
                }));

                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone)]
pub struct IdentityMap {
    inner: Rc<RefCell<IdentityMapInner>>,
}

impl IdentityMap {
    pub fn new<P, Q>(directory: P,
                     trash_directory: Q,
                     api: &sandstorm_api::Client<::capnp::any_pointer::Owned>)
                     -> Result<IdentityMap, Error>
        where P: AsRef<::std::path::Path>,
              Q: AsRef<::std::path::Path>,
    {
        // Create directories if they do not exist yet.
        ::std::fs::create_dir_all(&directory)?;
        ::std::fs::create_dir_all(&trash_directory)?;

        let (tx, poller) = ::multipoll::Poller::new(Box::new(Reaper));
        tokio::task::spawn_local(poller.map_err(|_|()));

        Ok(IdentityMap {
            inner: Rc::new(RefCell::new(IdentityMapInner {
                directory: directory.as_ref().to_path_buf(),
                trash_directory: trash_directory.as_ref().to_path_buf(),
                api: api.clone(),
                tasks: tx,
            })),
        })
    }

    pub fn put(&mut self, id: &[u8], identity: identity::Client) -> Result<(), Error> {
        let text_id = ::hex::encode(id);
        self.put_by_text(&text_id, identity)
    }

    pub fn put_by_text(&mut self, text_id: &str, identity: identity::Client) -> Result<(), Error> {
        if text_id.len() != 64 {
            return Err(Error::failed(format!("invalid identity ID {}", text_id)))
        }

        // truncate to 128 bits
        let truncated_text_id = &text_id[..32];

        let mut symlink = self.inner.borrow().directory.clone();
        symlink.push(&truncated_text_id);

        match ::std::fs::symlink_metadata(&symlink) {
            Err(ref e) if e.kind() == ::std::io::ErrorKind::NotFound => {
                IdentityMapInner::save_to_disk(
                    &self.inner,
                    truncated_text_id,
                    identity
                );
                Ok(())
            }
            Ok(_) => {
                let inner1 = self.inner.clone();
                let tti: String = truncated_text_id.into();
                let task = IdentityMapInner::read_from_disk(&self.inner, truncated_text_id);
                self.inner.borrow_mut().tasks.add(task.map_err(move |e| {
                    if e.kind == ::capnp::ErrorKind::Failed {
                        IdentityMapInner::save_to_disk(&inner1, &tti, identity);
                    }

                    e
                }).map_ok(|_| ()));

                Ok(())
            }
            Err(e) => {
                Err(e.into())
            }
        }
    }

    pub fn get(&mut self, id: &[u8]) -> Promise<identity::Client, Error> {
        if id.len() != 32 {
            return Promise::err(Error::failed(format!("invalid identity ID {:?}", id)))
        }

        let text_id = ::hex::encode(&id[..16]);
        self.get_by_text(&text_id)
    }

    pub fn get_by_text(&mut self, text_id: &str) -> Promise<identity::Client, Error> {
        if text_id.len() != 64 {
            return Promise::err(Error::failed(format!("invalid identity ID {}", text_id)))
        }

        // truncate to 128 bits
        let truncated_text_id = &text_id[..32];

        IdentityMapInner::read_from_disk(&self.inner, truncated_text_id)
    }

}
