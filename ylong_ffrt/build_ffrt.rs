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

//! build.rs file for ylong_ffrt

use std::fs;
use std::{env, path::PathBuf};

fn main() {
    let library_name = "ffrt";
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let library_dir = fs::canonicalize(root.join("lib")).unwrap();

    println!("cargo:rustc-link-lib={}", library_name);
    println!(
        "cargo:rustc-link-search=native={}",
        env::join_paths([library_dir]).unwrap().to_str().unwrap()
    );
    println!("cargo:rustc-link-lib=pthread");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
