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

//! An example for `tuple_form!()`

fn main() {
    #[derive(PartialEq, Debug)]
    enum Out {
        Finish,
        Fail,
    }

    // ylong_runtime_macro::tuple_form!(x with A except B at y);
    // tuple's length = number of `x`'s round brackets - 2
    // tuple's default element = `A`
    // tuple's except element = `B`
    // tuple's except element's index = `y`'s TokenTree count, in addition to the parentheses
    let tuple = ylong_runtime_macros::tuple_form!(( (((0)+1)+1) ) with Out::Fail except Out::Finish at ( ) );
    assert_eq!(tuple, (Out::Finish, Out::Fail));

    let tuple = ylong_runtime_macros::tuple_form!(( (((0)+1)+1) ) with Out::Fail except Out::Finish at ( _ ) );
    assert_eq!(tuple, (Out::Fail, Out::Finish));

    let tuple = ylong_runtime_macros::tuple_form!(( ((((0)+1)+1)+1) ) with Out::Fail except Out::Finish at ( _ _ ) );
    assert_eq!(tuple, (Out::Fail, Out::Fail, Out::Finish));

    let tuple = ylong_runtime_macros::tuple_form!(( (((((0)+1)+1)+1)+1) ) with Out::Fail except Out::Finish at ( _ _ _ ) );
    assert_eq!(tuple, (Out::Fail, Out::Fail, Out::Fail, Out::Finish));
}
