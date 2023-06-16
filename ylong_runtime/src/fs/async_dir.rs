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

use crate::fs::{async_op, poll_ready};
use crate::futures::poll_fn;
use crate::spawn::spawn_blocking;
use crate::{JoinHandle, TaskBuilder};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs::{FileType, Metadata};
use std::future::Future;
use std::io;
use std::iter::Fuse;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll::Ready;
use std::task::{Context, Poll};

const BLOCK_SIZE: usize = 32;

/// Creates a new directory at the given path.
///
/// The async version of [`std::fs::create_dir`]
///
/// # Errors
///
/// In the following situations, the function will return an error, but is not limited
/// to just these cases:
///
/// * The path has already been used.
/// * No permission to create directory at the given path.
/// * A parent directory in the path does not exist. In this case, use [`create_dir_all`] to create
///   the missing parent directory and the target directory at the same time.
///
/// # Examples
///
/// ```no_run
/// use std::io;
/// use ylong_runtime::fs;
/// async fn fs_func() -> io::Result<()> {
///     fs::create_dir("/parent/dir").await?;
///     Ok(())
/// }
/// ```
pub async fn create_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    async_op(move || std::fs::create_dir(path)).await
}

/// Creates a new directory and all of its missing parent directories.
///
/// The async version of [`std::fs::create_dir_all`]
///
/// # Errors
///
/// In the following situations, the function will return an error, but is not limited
/// to just these cases:
///
/// * The path has already been used.
/// * No permission to create directory at the given path.
/// * The missing parent directories can't not be created.
///
/// # Examples
///
/// ```no_run
/// use std::io;
/// use ylong_runtime::fs;
/// async fn fs_func() -> io::Result<()> {
///     fs::create_dir_all("/parent/dir").await?;
///     Ok(())
/// }
/// ```
pub async fn create_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    async_op(move || std::fs::create_dir_all(path)).await
}

/// Removes an empty directory.
///
/// The async version of [`std::fs::remove_dir`]
///
/// # Errors
///
/// In the following situations, the function will return an error, but is not limited
/// to just these cases:
///
/// * The directory does not exist.
/// * The given path is not a directory.
/// * No permission to remove directory at the given path.
/// * The directory isn't empty.
///
/// # Examples
///
/// ```no_run
/// use std::io;
/// use ylong_runtime::fs;
/// async fn fs_func() -> io::Result<()> {
///     fs::remove_dir("/parent/dir").await?;
///     Ok(())
/// }
/// ```
pub async fn remove_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    async_op(move || std::fs::remove_dir(path)).await
}

/// Removes a directory and all its contents at the given path.
///
/// The async version of [`std::fs::remove_dir_all`]
///
/// # Errors
///
/// * The directory does not exist.
/// * The given path is not a directory.
/// * No permission to remove directory or its contents at the given path.
///
/// # Examples
///
/// ```no_run
/// use std::io;
/// use ylong_runtime::fs;
/// async fn fs_func() -> io::Result<()> {
///     fs::remove_dir_all("/parent/dir").await?;
///     Ok(())
/// }
/// ```
pub async fn remove_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    async_op(move || std::fs::remove_dir_all(path)).await
}

/// Returns an iterator over the entries within a directory.
///
/// The async version of [`std::fs::read_dir`]
///
/// # Errors
///
/// * The directory does not exist.
/// * The given path is not a directory.
/// * No permission to view the contents at the given path.
///
/// # Examples
///
/// ```no_run
/// use std::io;
/// use ylong_runtime::fs;
/// async fn fs_func() -> io::Result<()> {
///     let mut dir = fs::read_dir("/parent/dir").await?;
///     assert!(dir.next().await.is_ok());
///     Ok(())
/// }
/// ```
pub async fn read_dir<P: AsRef<Path>>(path: P) -> io::Result<ReadDir> {
    let path = path.as_ref().to_owned();
    async_op(|| {
        let mut std_dir = std::fs::read_dir(path)?.fuse();
        let mut block = VecDeque::with_capacity(BLOCK_SIZE);
        ReadDir::fill_block(&mut std_dir, &mut block);
        Ok(ReadDir::new(std_dir, block))
    })
    .await
}

type Entries = (Fuse<std::fs::ReadDir>, VecDeque<io::Result<DirEntry>>);

enum State {
    Available(Box<Option<Entries>>),
    Empty(JoinHandle<Entries>),
}
/// Directory for reading file entries.
///
/// Returned from the [`read_dir`] function of this module and
/// will yield instances of [`io::Result`]<[`DirEntry`]>. A [`DirEntry`]
/// contains information like the entry's path and possibly other metadata.
///
/// # Errors
///
/// Returns [`Err`] if an IO error occurs during iteration.
pub struct ReadDir(State);

impl ReadDir {
    fn new(std_dir: Fuse<std::fs::ReadDir>, block: VecDeque<io::Result<DirEntry>>) -> ReadDir {
        ReadDir(State::Available(Box::new(Some((std_dir, block)))))
    }

    fn fill_block(
        std_dir: &mut Fuse<std::fs::ReadDir>,
        block: &mut VecDeque<io::Result<DirEntry>>,
    ) {
        for res in std_dir.by_ref().take(BLOCK_SIZE) {
            match res {
                Ok(entry) => block.push_back(Ok(DirEntry(Arc::new(entry)))),
                Err(e) => block.push_back(Err(e)),
            }
        }
    }

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<Option<DirEntry>>> {
        loop {
            match self.0 {
                State::Available(ref mut dir) => {
                    let (mut std_dir, mut block) = dir.take().unwrap();
                    match block.pop_front() {
                        Some(Ok(entry)) => {
                            self.0 = State::Available(Box::new(Some((std_dir, block))));
                            return Ready(Ok(Some(entry)));
                        }
                        Some(Err(e)) => {
                            self.0 = State::Available(Box::new(Some((std_dir, block))));
                            return Ready(Err(e));
                        }
                        None => {}
                    }

                    self.0 = State::Empty(spawn_blocking(&TaskBuilder::new(), move || {
                        ReadDir::fill_block(&mut std_dir, &mut block);
                        (std_dir, block)
                    }));
                }
                State::Empty(ref mut handle) => {
                    let (std_dir, mut block) = poll_ready!(Pin::new(handle).poll(cx))?;
                    let res = match block.pop_front() {
                        Some(Ok(entry)) => Ok(Some(entry)),
                        Some(Err(e)) => Err(e),
                        None => Ok(None),
                    };
                    self.0 = State::Available(Box::new(Some((std_dir, block))));
                    return Ready(res);
                }
            }
        }
    }

    /// Returns the next entry in the directory.
    ///
    /// # Return value
    /// The function returns:
    /// * `Ok(Some(entry))` entry is an entry in the directory.
    /// * `Ok(None)` if there is no more entries in the directory.
    /// * `Err(e)` if an IO error occurred.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use ylong_runtime::fs;
    /// async fn fs_func() -> io::Result<()> {
    ///     let mut dir = fs::read_dir("/parent/dir").await?;
    ///     assert!(dir.next().await.is_ok());
    ///     Ok(())
    /// }
    /// ```
    pub async fn next(&mut self) -> io::Result<Option<DirEntry>> {
        poll_fn(|cx| self.poll_next(cx)).await
    }
}

/// Entries returned by the [`ReadDir::next_entry`].
///
/// Represents an entry inside of a directory on the filesystem.
/// Each entry can be inspected via methods to learn about the full path
/// or possibly other metadata through per-platform extension traits.
pub struct DirEntry(Arc<std::fs::DirEntry>);

impl DirEntry {
    /// Returns the full path to the file represented by this entry.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use ylong_runtime::fs;
    ///
    /// async fn fs_func() -> io::Result<()> {
    ///     let mut dir = fs::read_dir("/parent/dir").await?;
    ///     while let Some(entry) = dir.next().await? {
    ///          println!("{:?}", entry.path());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    ///
    /// This prints output like:
    ///
    /// ```text
    /// "/parent/dir/some.txt"
    /// "/parent/dir/rust.rs"
    /// ```
    pub fn path(&self) -> PathBuf {
        self.0.path()
    }

    /// Returns the name of the file represented by this entry.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use ylong_runtime::fs;
    ///
    /// async fn fs_func() -> io::Result<()>{
    ///     let mut dir = fs::read_dir("/parent/dir").await?;
    ///     while let Some(entry) = dir.next().await? {
    ///          println!("{:?}", entry.file_name());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub fn file_name(&self) -> OsString {
        self.0.file_name()
    }

    /// Returns the metadata for the file represented by this entry.
    ///
    /// This function won't traverse symlinks if this entry points
    /// at a symlink.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use ylong_runtime::fs;
    ///
    /// async fn fs_func() -> io::Result<()> {
    ///     let mut dir = fs::read_dir("/parent/dir").await?;
    ///     while let Some(entry) = dir.next().await? {
    ///          if let Ok(metadata) = entry.metadata().await {
    ///             println!("{:?}", metadata.permissions());
    ///          }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn metadata(&self) -> io::Result<Metadata> {
        let entry = self.0.clone();
        async_op(move || entry.metadata()).await
    }

    /// Returns the file type for the file represented by this entry.
    ///
    /// This function won't traverse symlinks if this entry points
    /// at a symlink.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use ylong_runtime::fs;
    ///
    /// async fn fs_func() -> io::Result<()> {
    ///     let mut dir = fs::read_dir("/parent/dir").await?;
    ///     while let Some(entry) = dir.next().await? {
    ///          if let Ok(file_type) = entry.file_type().await {
    ///             println!("{:?}", file_type);
    ///          }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn file_type(&self) -> io::Result<FileType> {
        let entry = self.0.clone();
        async_op(move || entry.file_type()).await
    }
}
