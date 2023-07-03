// Copyright (c) 2023 Huawei Device Co., Ltd.
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::net::ScheduleIO;
use std::io;
use std::ops::Deref;
use ylong_io::{Interest, Source};
use crate::macros::cfg_io;
use crate::util::slab::Ref;

cfg_io!(
    use std::task::{Context, Poll};
    use std::mem::MaybeUninit;
    use crate::io::ReadBuf;
    use crate::net::ReadyEvent;
    #[cfg(not(feature = "ffrt"))]
    use crate::executor::worker::{get_current_ctx, WorkerContext};
    use std::io::{Read, Write};
);

/// Wrapper that turns a sync `Source` io into an async one. This struct interacts with the reactor
/// of the runtime.
pub(crate) struct AsyncSource<E: Source> {
    /// Sync io that implements `Source` trait.
    io: Option<E>,

    /// Entry list of the runtime's reactor, `AsyncSource` object will be registered into it
    /// when created.
    pub(crate) entry: Ref<ScheduleIO>,
}

impl<E: Source> AsyncSource<E> {
    /// Wraps a `Source` object into an `AsyncSource`. When the `AsyncSource` object is created,
    /// it's fd will be registered into runtime's reactor.
    ///
    /// If `interest` passed in is None, the interested event for fd registration will be both
    /// readable and writable.
    ///
    /// # Error
    ///
    /// If no reactor is found or fd registration fails, an error will be returned.
    ///
    pub fn new(mut io: E, interest: Option<Interest>) -> io::Result<AsyncSource<E>> {
        #[cfg(not(feature = "ffrt"))]
        let inner = {
            let context = get_current_ctx().ok_or(io::Error::new(io::ErrorKind::Other, "get_current_ctx() fail"))?;
            match context {
                WorkerContext::Multi(ctx) => &ctx.handle,
                WorkerContext::Curr(ctx) => &ctx.handle,
            }
        };
        #[cfg(feature = "ffrt")]
        let inner = crate::net::Handle::get_ref();
        // let inner = Driver::inner();
        let interest = interest.unwrap_or_else(|| Interest::WRITABLE.add(Interest::READABLE));
        let entry = inner.register_source(&mut io, interest)?;
        Ok(AsyncSource {
            io: Some(io),
            entry,
        })
    }

    /// Asynchronously waits for events to happen. If the io returns `EWOULDBLOCK`, the readiness
    /// of the io will be reset. Otherwise, the corresponding event will be returned.
    pub(crate) async fn async_process<F, R>(&self, interest: Interest, mut op: F) -> io::Result<R>
    where
        F: FnMut() -> io::Result<R>,
    {
        loop {
            let ready = self.entry.readiness(interest).await?;
            match op() {
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.entry.clear_readiness(ready);
                }
                x => return x,
            }
        }
    }

    cfg_io! {
        pub(crate) fn poll_ready(
            &self,
            cx: &mut Context<'_>,
            interest: Interest,
        ) -> Poll<io::Result<ReadyEvent>> {
            let ready = self.entry.poll_readiness(cx, interest);
            let x = match ready {
                Poll::Ready(x) => x,
                Poll::Pending => return Poll::Pending,
            };

            Poll::Ready(Ok(x))
        }

        pub(crate) fn poll_io<R>(
            &self,
            cx: &mut Context<'_>,
            interest: Interest,
            mut f: impl FnMut() -> io::Result<R>,
        ) -> Poll<io::Result<R>> {
            loop {
                let ready = match self.poll_ready(cx, interest) {
                    Poll::Ready(Ok(x)) => x,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                };

                match f() {
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        self.entry.clear_readiness(ready);
                    }
                    x => return Poll::Ready(x),
                }
            }
        }

        pub(crate) fn try_io<R> (
            &self,
            interest: Interest,
            mut f: impl FnMut() -> io::Result<R>,
        ) -> io::Result<R> {
            let event = self.entry.get_readiness(interest);

            if event.ready.is_empty() {
                return Err(io::ErrorKind::WouldBlock.into());
            }

            match f() {
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.entry.clear_readiness(event);
                    Err(io::ErrorKind::WouldBlock.into())
                }
                res => res,
            }
        }

        pub(crate) fn poll_read_io<R>(
            &self,
            cx: &mut Context<'_>,
            f: impl FnMut() -> io::Result<R>,
        ) -> Poll<io::Result<R>> {
            self.poll_io(cx, Interest::READABLE, f)
        }

        pub(crate) fn poll_write_io<R>(
            &self,
            cx: &mut Context<'_>,
            f: impl FnMut() -> io::Result<R>,
        ) -> Poll<io::Result<R>> {
            self.poll_io(cx, Interest::WRITABLE, f)
        }

        pub(crate) fn poll_read<'a>(
            &'a self,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>>
        where
            &'a E: io::Read + 'a,
        {
            let ret = self.poll_io(cx, Interest::READABLE, || unsafe {
                let slice = &mut *(buf.unfilled_mut() as *mut [MaybeUninit<u8>] as *mut [u8]);
                self.io.as_ref().unwrap().read(slice)
            });
            let r_len = match ret {
                Poll::Ready(Ok(x)) => x,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };
            buf.assume_init(r_len);
            buf.advance(r_len);

            Poll::Ready(Ok(()))
        }

        pub(crate) fn poll_write<'a>(
            &'a self,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>>
        where
            &'a E: io::Write + 'a,
        {
            self.poll_io(cx, Interest::WRITABLE, || {
                self.io.as_ref().unwrap().write(buf)
            })
        }

        pub(crate) fn poll_write_vectored<'a>(
            &'a self,
            cx: &mut Context<'_>,
            bufs: &[io::IoSlice<'_>],
        ) -> Poll<io::Result<usize>>
        where
            &'a E: io::Write + 'a,
        {
            self.poll_io(cx, Interest::WRITABLE, || {
                self.io.as_ref().unwrap().write_vectored(bufs)
            })
        }
    }
}

impl<E: Source> Deref for AsyncSource<E> {
    type Target = E;

    fn deref(&self) -> &Self::Target {
        self.io.as_ref().unwrap()
    }
}

// Deregisters fd when the `AsyncSource` object get dropped.
impl<E: Source> Drop for AsyncSource<E> {
    fn drop(&mut self) {
        if let Some(mut io) = self.io.take() {
            #[cfg(not(feature = "ffrt"))]
            let inner = {
                let context = get_current_ctx().expect("AsyncSource drop get_current_ctx() fail");
                match context {
                    WorkerContext::Multi(ctx) => &ctx.handle,
                    WorkerContext::Curr(ctx) => &ctx.handle,
                }
            };
            #[cfg(feature = "ffrt")]
            let inner = crate::net::Handle::get_ref();
            let _ = inner.deregister_source(&mut io);
        }
    }
}