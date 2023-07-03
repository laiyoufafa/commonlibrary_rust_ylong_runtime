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

use ylong_runtime::fs::{create_dir, create_dir_all, read_dir, remove_dir, remove_dir_all, File};

/// SDV test for directory operations.
///
/// # Title
/// sdv_async_dir
///
/// # Brief
/// 1.Create a new directory.
/// 2.Create two files to read.
/// 3.Read the directory and check the name of files.
/// 4.Delete the directory and files in it.
#[test]
fn sdv_async_dir() {
    let handle = ylong_runtime::spawn(async move {
        let _ = create_dir("./tests/dir_test").await;
        File::create("./tests/dir_test/test1.txt").await.unwrap();
        File::create("./tests/dir_test/test2.txt").await.unwrap();
        let mut dir = read_dir("./tests/dir_test").await.unwrap();
        let entry = dir.next().await.unwrap().unwrap();
        assert!(!entry.file_type().await.unwrap().is_dir());
        assert!(entry.file_type().await.unwrap().is_file());
        assert!(entry.file_name().into_string().unwrap().contains("test"));
        let entry = dir.next().await.unwrap().unwrap();
        assert!(!entry.metadata().await.unwrap().is_dir());
        assert!(entry.metadata().await.unwrap().is_file());
        assert!(!entry.metadata().await.unwrap().permissions().readonly());
        assert!(entry.file_name().into_string().unwrap().contains("test"));
        assert!(dir.next().await.unwrap().is_none());
        assert!(remove_dir_all("./tests/dir_test").await.is_ok());
    });
    ylong_runtime::block_on(handle).unwrap();
}

/// SDV test for creating and removing directories.
///
/// # Title
/// sdv_async_dir_create_remove
///
/// # Brief
/// 1.Create a new directory at the given path.
/// 2.Create a new directory at the given path that is used.
/// 3.Create a new directory at the given path and whose parent directory is missing.
/// 4.Create a new directory and all of its parents.
/// 5.Remove a directory at the given path.
/// 6.Remove a directory that does not exist.
/// 7.Remove a directory that is not a directory.
/// 8.Remove a directory that is not empty.
/// 9.Remove a directory and all of its contents.
#[test]
fn sdv_async_dir_create_remove() {
    let handle = ylong_runtime::spawn(async move {
        assert!(create_dir("./tests/dir_test1").await.is_ok());
        assert!(create_dir("./tests/dir_test1").await.is_err());
        assert!(create_dir("./tests/dir_test2/dir_test_child")
            .await
            .is_err());
        assert!(create_dir_all("./tests/dir_test2/dir_test_child")
            .await
            .is_ok());
        assert!(remove_dir("./tests/dir_test1").await.is_ok());
        assert!(remove_dir("./tests/dir_test1").await.is_err());
        assert!(remove_dir("./tests/async_dir").await.is_err());
        assert!(remove_dir("./tests/dir_test2").await.is_err());
        assert!(remove_dir_all("./tests/dir_test2").await.is_ok());
    });
    ylong_runtime::block_on(handle).unwrap();
}
