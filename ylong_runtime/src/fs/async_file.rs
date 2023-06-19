/*
 * Copyright (c) 2023 Huawei Device Co., Ltd.
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::fs::file_buf::FileBuf;
use crate::fs::{async_op, poll_ready};
use crate::futures::poll_fn;
use crate::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use crate::spawn::spawn_blocking;
use crate::sync::Mutex;
use crate::task::{JoinHandle, TaskBuilder};
use std::fs::{File as SyncFile, Metadata, Permissions};
use std::future::Future;
use std::io;
use std::io::{Seek, SeekFrom};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// An asynchronous wrapping of [`std::fs::File`]. Provides async read/write methods.
pub struct File {
    file: Arc<SyncFile>,
    inner: Mutex<FileInner>,
}

struct FileInner {
    state: FileState,
    write_err: Option<io::ErrorKind>,
    idx: u64,
}

type RWJoinHandle = JoinHandle<(FileBuf, io::Result<()>)>;

type SeekJoinHandle = JoinHandle<(FileBuf, io::Result<u64>)>;

enum FileState {
    Idle(Option<FileBuf>),
    Reading(RWJoinHandle),
    Writing(RWJoinHandle),
    Seeking(SeekJoinHandle),
}

enum FileOp {
    Reading,
    Writing,
    Seeking,
}

impl Future for FileState {
    type Output = io::Result<u64>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = self.get_mut();
        match state {
            FileState::Idle(_) => unreachable!(),
            FileState::Reading(x) | FileState::Writing(x) => {
                let (file_buf, res) = poll_ready!(Pin::new(x).poll(cx))?;
                *state = FileState::Idle(Some(file_buf));
                // For read and write, we dont care about the output
                Poll::Ready(res.map(|_| 0_u64))
            }
            FileState::Seeking(x) => {
                let (file_buf, res) = poll_ready!(Pin::new(x).poll(cx))?;
                *state = FileState::Idle(Some(file_buf));
                Poll::Ready(res)
            }
        }
    }
}

impl FileState {
    #[inline]
    fn get_op(&self) -> FileOp {
        match self {
            FileState::Idle(_) => unreachable!(),
            FileState::Reading(_) => FileOp::Reading,
            FileState::Writing(_) => FileOp::Writing,
            FileState::Seeking(_) => FileOp::Seeking,
        }
    }
}

impl File {
    /// Creates a new [`File`] struct.
    pub fn new(file: SyncFile) -> File {
        File {
            file: Arc::new(file),
            inner: Mutex::new(FileInner {
                state: FileState::Idle(Some(FileBuf::with_capacity(0))),
                write_err: None,
                idx: 0,
            }),
        }
    }

    /// Attempts to open a file in read-only mode asynchronously.
    ///
    /// See the [`super::OpenOptions::open`] method for more details.
    ///
    /// # Errors
    ///
    /// This function will return an error if `path` does not already exist.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn open() -> std::io::Result<()> {
    ///     let mut f = File::open("foo.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn open<P: AsRef<Path>>(path: P) -> io::Result<File> {
        let path = path.as_ref().to_owned();
        let file = async_op(|| SyncFile::open(path)).await?;
        Ok(File::new(file))
    }

    /// Opens a file in write-only mode asynchronously.
    ///
    /// This function will create a file if it does not exist
    /// and truncate it if it does.
    ///
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn create() -> std::io::Result<()> {
    ///     let mut f = File::create("foo.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn create<P: AsRef<Path>>(path: P) -> io::Result<File> {
        let path = path.as_ref().to_owned();
        let file = async_op(|| SyncFile::create(path)).await?;
        Ok(File::new(file))
    }

    /// Changes the permissions on the underlying file asynchronously.
    ///
    /// # Errors
    /// This function will return an error if the user lacks permission change
    /// attributes on the underlying file. It may also return an error in other
    /// os-specific unspecified cases.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn set_permissions() -> std::io::Result<()> {
    ///     let file = File::open("foo.txt").await?;
    ///     let mut perms = file.metadata().await?.permissions();
    ///     perms.set_readonly(true);
    ///     file.set_permissions(perms).await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// Note that this method alters the permissions of the underlying file,
    /// even though it takes `&self` rather than `&mut self`.
    pub async fn set_permissions(&self, perm: Permissions) -> io::Result<()> {
        let file = self.file.clone();
        async_op(move || file.set_permissions(perm)).await
    }

    /// Attempts to sync all OS-internal metadata to disk asynchronously.
    ///
    /// This function will attempt to ensure that all in-memory data reaches the
    /// filesystem before returning.
    ///
    /// This can be used to handle errors that would otherwise only be caught
    /// when the `File` is closed. Dropping a file will ignore errors in
    /// synchronizing this in-memory data.

    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::File;
    /// use ylong_runtime::io::AsyncWriteExt;
    ///
    /// async fn sync_all() -> std::io::Result<()> {
    ///     let mut f = File::create("foo.txt").await?;
    ///     f.write_all(b"Hello, world!").await?;
    ///
    ///     f.sync_all().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn sync_all(&self) -> io::Result<()> {
        let mut file = self.inner.lock().await;
        if let Err(e) = poll_fn(|cx| Pin::new(&mut *file).poll_flush(cx)).await {
            file.write_err = Some(e.kind());
        }
        let file = self.file.clone();
        async_op(move || file.sync_all()).await
    }

    /// This function is similar to [`File::sync_all`], except that it might not
    /// synchronize file metadata to the filesystem.
    ///
    /// This is intended for use cases that must synchronize content, but don't
    /// need the metadata on disk. The goal of this method is to reduce disk
    /// operations.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::File;
    /// use ylong_runtime::io::AsyncWriteExt;
    ///
    /// async fn sync_data() -> std::io::Result<()> {
    ///     let mut f = File::create("foo.txt").await?;
    ///     f.write_all(b"Hello, world!").await?;
    ///
    ///     f.sync_data().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn sync_data(&self) -> io::Result<()> {
        let mut file = self.inner.lock().await;
        if let Err(e) = poll_fn(|cx| Pin::new(&mut *file).poll_flush(cx)).await {
            file.write_err = Some(e.kind());
        }
        let file = self.file.clone();
        async_op(move || file.sync_data()).await
    }

    /// Queries metadata about the underlying file asynchronously.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn metadata() -> std::io::Result<()> {
    ///     let mut f = File::open("foo.txt").await?;
    ///     let metadata = f.metadata().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn metadata(&self) -> io::Result<Metadata> {
        let file = self.file.clone();
        async_op(move || file.metadata()).await
    }
}

impl AsyncSeek for File {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        let file = self.get_mut();
        let inner = file.inner.get_mut();
        loop {
            match inner.state {
                FileState::Idle(ref mut buf) => {
                    let mut r_buf = buf.take().unwrap();

                    // move the cursor back since there's unread data in the buf
                    let unread = r_buf.drop_unread();
                    if unread != 0 {
                        if let SeekFrom::Current(ref mut idx) = pos {
                            *idx -= unread
                        }
                    }

                    let file = file.file.clone();
                    inner.state =
                        FileState::Seeking(spawn_blocking(&TaskBuilder::new(), move || {
                            let ret = (&*file).seek(pos);
                            (r_buf, ret)
                        }));
                }
                ref mut state => {
                    let op = state.get_op();
                    let res = poll_ready!(Pin::new(state).poll(cx));
                    match op {
                        FileOp::Reading => {}
                        FileOp::Writing => {
                            if let Err(e) = res {
                                // Save the error for the next write.
                                inner.write_err = Some(e.kind());
                            }
                        }
                        FileOp::Seeking => {
                            if let Ok(idx) = res {
                                inner.idx = idx;
                            }
                            return Poll::Ready(res);
                        }
                    }
                }
            }
        }
    }
}

impl AsyncRead for File {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let file = self.get_mut();
        let inner = file.inner.get_mut();

        loop {
            match inner.state {
                FileState::Idle(ref mut file_buf) => {
                    let mut r_buf = file_buf.take().unwrap();
                    // There is still remaining data from the last read, append it to read buf directly
                    if r_buf.remaining() != 0 {
                        r_buf.append_to(buf);
                        *file_buf = Some(r_buf);
                        return Poll::Ready(Ok(()));
                    }

                    // Make sure there is enough space to read. File_buf's size might be bigger than
                    // the read_buf's size since other thread might also read into the read_buf.
                    r_buf.reserve(buf.remaining());

                    // State transition
                    let file = file.file.clone();
                    inner.state =
                        FileState::Reading(spawn_blocking(&TaskBuilder::new(), move || {
                            let ret = r_buf.read(&mut &*file).map(|_| ());
                            (r_buf, ret)
                        }));
                }
                FileState::Reading(ref mut x) => {
                    let (mut file_buf, res) = poll_ready!(Pin::new(x).poll(cx))?;
                    // Append the data inside the file to the read buffer
                    if res.is_ok() {
                        file_buf.append_to(buf);
                    }
                    inner.state = FileState::Idle(Some(file_buf));
                    return Poll::Ready(res);
                }
                FileState::Writing(ref mut x) => {
                    let (file_buf, res) = poll_ready!(Pin::new(x).poll(cx))?;

                    // Save the error for the next write
                    if let Err(e) = res {
                        inner.write_err = Some(e.kind());
                    }
                    inner.state = FileState::Idle(Some(file_buf))
                }
                FileState::Seeking(ref mut x) => {
                    let (file_buf, res) = poll_ready!(Pin::new(x).poll(cx))?;
                    inner.state = FileState::Idle(Some(file_buf));
                    if let Ok(idx) = res {
                        inner.idx = idx;
                    }
                }
            }
        }
    }
}

impl AsyncWrite for File {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let file = self.get_mut();
        let inner = file.inner.get_mut();

        if let Some(e) = inner.write_err {
            return Poll::Ready(Err(e.into()));
        }

        loop {
            match inner.state {
                FileState::Idle(ref mut file_buf) => {
                    let mut w_buf = file_buf.take().unwrap();

                    let unread = w_buf.drop_unread();
                    let n = w_buf.append(buf);
                    let file = file.file.clone();

                    inner.state =
                        FileState::Writing(spawn_blocking(&TaskBuilder::new(), move || {
                            // Move the cursor back since there's unread data in the buf.
                            if unread != 0 {
                                if let Err(e) = (&*file).seek(SeekFrom::Current(-unread)) {
                                    return (w_buf, Err(e));
                                }
                            }
                            let res = w_buf.write(&mut &*file);
                            (w_buf, res)
                        }));
                    return Poll::Ready(Ok(n));
                }
                ref mut state => {
                    let op = state.get_op();
                    if let Poll::Ready(Err(e)) = Pin::new(state).poll(cx) {
                        if let FileOp::Writing = op {
                            return Poll::Ready(Err(e));
                        }
                    }
                }
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.get_mut().poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl FileInner {
    fn poll_flush(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        if let Some(e) = self.write_err {
            return Poll::Ready(Err(e.into()));
        }

        match self.state {
            FileState::Idle(_) => Poll::Ready(Ok(())),
            ref mut state => {
                let op = state.get_op();
                let res = poll_ready!(Pin::new(state).poll(cx));

                match op {
                    FileOp::Writing => Poll::Ready(res.map(|_| ())),
                    _ => Poll::Ready(Ok(())),
                }
            }
        }
    }
}
