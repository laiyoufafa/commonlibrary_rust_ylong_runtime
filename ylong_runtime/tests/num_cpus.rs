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

use ylong_runtime::util::num_cpus::get_cpu_num;

#[cfg(target_os = "linux")]
#[test]
fn sdv_num_cpus_linux_test() {
    use ylong_runtime::util::num_cpus::linux::get_cpu_num_configured;

    let cpus = get_cpu_num();
    assert!(cpus > 0);
    let cpus = get_cpu_num_configured();
    assert!(cpus > 0);
}

#[cfg(target_os = "windows")]
#[test]
fn sdv_num_cpus_windows_test() {
    let cpus = get_cpu_num();
    assert!(cpus > 0);
}
