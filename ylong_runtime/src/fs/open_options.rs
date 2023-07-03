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

use crate::fs::{async_op, File};
use std::fs::OpenOptions as SyncOpenOptions;
use std::io;
use std::path::Path;

/// An asynchronous version of the [`std::fs::OpenOptions`];
///
/// Options and flags which can be used to configure how a file is opened.
///
/// ```no_run
/// use ylong_runtime::fs::OpenOptions;
///
/// async fn open_with_options() {
///     let file = OpenOptions::new()
///             .read(true)
///             .write(true)
///             .create(true)
///             .open("foo.txt").await;
/// }
/// ```
#[derive(Clone, Debug)]
pub struct OpenOptions(SyncOpenOptions);

impl OpenOptions {
    /// Creates a blank new set of options ready for configuration when opening a file.
    ///
    /// All options are initially set to `false` just like [`std::fs::OpenOptions::new`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::OpenOptions;
    ///
    /// async fn create_option() {
    ///     let mut options = OpenOptions::new();
    ///     let file = options.read(true).open("foo.txt").await;
    /// }
    /// ```
    // Prevent to increase binary size and thus mask this warning.
    #[allow(clippy::new_without_default)]
    pub fn new() -> OpenOptions {
        OpenOptions(SyncOpenOptions::new())
    }

    /// Sets the option for file read access.
    ///
    /// This option, when true, will indicate that the file should be
    /// `read`-able if opened.
    ///
    /// This method's behavior is the same as [`std::fs::OpenOptions::read`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::OpenOptions;
    ///
    /// async fn open_with_read() {
    ///     let file = OpenOptions::new().read(true).open("foo.txt").await;
    /// }
    /// ```
    pub fn read(&mut self, read: bool) -> &mut OpenOptions {
        self.0.read(read);
        self
    }

    /// Sets the option for file write access.
    ///
    /// This option, when true, will indicate that the file should be
    /// `write`-able if opened.
    ///
    /// If the file already exists, any write calls on it will overwrite its previous
    /// contents, without truncating it.
    ///
    /// This method's behavior is the same as [`std::fs::OpenOptions::write`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::OpenOptions;
    ///
    /// async fn open_with_write() {
    ///     let file = OpenOptions::new().write(true).open("foo.txt").await;
    /// }
    /// ```
    pub fn write(&mut self, write: bool) -> &mut OpenOptions {
        self.0.write(write);
        self
    }

    /// Sets the option for the file append mode.
    ///
    /// This option, when true, means that writes will append to a file instead
    /// of overwriting previous contents.
    ///
    /// Note that when setting `.append(true)`, file write access will also be turned on
    ///
    /// For most filesystems, the operating system guarantees that all writes are
    /// atomic: no writes get mangled because another thread or process writes at the same
    /// time.
    ///
    /// User should write all data that belongs together in one operation during appending.
    /// This can be done by concatenating strings before passing them to [`write()`],
    /// or using a buffered writer (with a buffer of adequate size),
    /// and calling [`flush()`] when the message is written completely.
    ///
    /// If a file is opened with both read and append access, beware that after
    /// opening and every write, the position for reading may be set at the end of the file.
    /// So, before writing, save the current position, and restore it before the next read.
    ///
    /// This method's behavior is the same as [`std::fs::OpenOptions::append`].
    ///
    /// ## Note
    ///
    /// This method doesn't create the file if it doesn't exist. Use the
    /// [`OpenOptions::create`] method to do so.
    ///
    /// [`write()`]: crate::io::AsyncWriteExt::write
    /// [`flush()`]: crate::io::AsyncWrite::poll_flush
    /// [seek]: crate::io::AsyncSeekExt::seek
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::OpenOptions;
    ///
    /// async fn open_with_append() {
    ///     let file = OpenOptions::new().append(true).open("foo.txt").await;
    /// }
    /// ```
    pub fn append(&mut self, append: bool) -> &mut OpenOptions {
        self.0.append(append);
        self
    }

    /// Sets the option for truncating a file's previous content.
    ///
    /// If a file is successfully opened with this option set, it will truncate
    /// the file to 0 length if it already exists. Any already-existed content in
    /// this file will be dropped.
    ///
    /// The file must be opened with write access for truncate to work, which is different
    /// from append mode.
    ///
    /// This method's behavior is the same as [`std::fs::OpenOptions::truncate`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::OpenOptions;
    ///
    /// async fn open_with_truncate() {
    ///     let file = OpenOptions::new().write(true).truncate(true).open("foo.txt").await;
    /// }
    /// ```
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.0.truncate(truncate);
        self
    }

    /// Sets the option to create a new file if it doesn't already exist, or simply open
    /// it if it does exist.
    ///
    /// In order for the file to be created, [`OpenOptions::write`] or
    /// [`OpenOptions::append`] access must be set to true.
    ///
    /// This method's behavior is the same as [`std::fs::OpenOptions::create`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::OpenOptions;
    ///
    /// async fn open_with_create() {
    ///     let file = OpenOptions::new().write(true).create(true).open("foo.txt").await;
    /// }
    /// ```
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.0.create(create);
        self
    }

    /// Sets the option to create a new file.
    ///
    /// If the file already exists, opening the file with the option set will cause an error.
    ///
    /// No file is allowed to exist at the target location, also no (dangling) symlink. In this
    /// way, if the call succeeds, the file returned is guaranteed to be new.
    ///
    /// This option guarantees the operation of checking whether a file exists and creating
    /// a new one is atomic.
    ///
    /// If `.create_new(true)` is set, [`.create()`] and [`.truncate()`] are
    /// ignored.
    ///
    /// The file must be opened with write or append mode in order to create
    /// a new file.
    ///
    /// [`.create(true)`]: OpenOptions::create
    /// [`.truncate(true)`]: OpenOptions::truncate
    ///
    /// This method's behavior is the same as [`std::fs::OpenOptions::create_new`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::OpenOptions;
    ///
    /// async fn open_with_create_new() {
    ///     let file = OpenOptions::new().write(true).create_new(true).open("foo.txt").await;
    /// }
    /// ```
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.0.create_new(create_new);
        self
    }

    /// Asynchronously opens a file at `path` with the options specified by `self`.
    ///
    /// # Errors
    ///
    /// This method's behavior is the same as [`std::fs::OpenOptions::open`].
    ///
    /// * [`NotFound`]: The specified file does not exist and neither `create` or
    ///   `create_new` is set.
    /// * [`NotFound`]: One of the directory components of the file path doesn't exist.
    /// * [`PermissionDenied`]: The user lacks permission to get the specified access
    ///   rights for the file.
    /// * [`PermissionDenied`]: The user lacks permission to open one of the directory
    ///   components of the specified path.
    /// * [`AlreadyExists`]: `create_new` was specified and the file already exists.
    /// * [`InvalidInput`]: Invalid combinations of open options (truncate without
    ///   write access, no access mode set, etc.).
    ///
    /// The following errors don't match any existing [`io::ErrorKind`] at the moment:
    /// * One of the directory components of the specified file path was not,
    ///   in fact, a directory.
    /// * Filesystem-level errors: full disk, write permission requested on a read-only
    ///   file system, exceeded disk quota, too many open files, too long filename,
    ///   too many symbolic links in the specified path (Unix-like systems only), etc.
    ///
    /// [`NotFound`]: std::io::ErrorKind::NotFound
    /// [`PermissionDenied`]: std::io::ErrorKind::PermissionDenied
    /// [`AlreadyExists`]: std::io::ErrorKind::AlreadyExists
    /// [`InvalidInput`]: std::io::ErrorKind::InvalidInput
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_runtime::fs::OpenOptions;
    ///
    /// async fn option_open() {
    ///     let file = OpenOptions::new().read(true).open("foo.txt").await;
    /// }
    /// ```
    pub async fn open<P: AsRef<Path>>(&self, path: P) -> io::Result<File> {
        let path = path.as_ref().to_owned();
        let options = self.0.clone();
        let file = async_op(move || options.open(path)).await?;
        Ok(File::new(file))
    }
}
#[cfg(all(test, target_os = "linux"))]
mod test {
    use crate::fs::OpenOptions;

    /// UT test for Openoption.
    ///
    /// # Title
    /// ut_set_openoption
    ///
    /// # Brief
    /// 1. Call `read`、`write`、`append`、`truncate`、`create`、`create_new` function, passing in the specified parameters.
    /// 2. Check if the settings are correct.
    #[cfg(target_os = "linux")]
    #[test]
    fn ut_set_openoption() {
        let mut option = OpenOptions::new();
        option.read(true);
        option.write(true);
        option.append(true);
        option.truncate(true);
        option.create(true);
        option.create_new(true);
        assert_eq!("OpenOptions(OpenOptions { read: true, write: true, append: true, truncate: true, create: true, create_new: true, custom_flags: 0, mode: 438 })", format!("{:?}", option.0))
    }
}
