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

use proc_macro::{TokenStream, TokenTree};

/// Convert [`TokenStream`] to [`TupleParser`]
pub(crate) fn tuple_parser(input: TokenStream) -> TupleParser {
    let buf = input.into_iter().collect::<Vec<_>>();

    let mut flag_with = false;
    let mut flag_except = false;
    let mut flag_at = false;

    let mut len = 0;
    let mut default = TokenStream::new();
    let mut except = TokenStream::new();
    let mut except_index = 0;

    let mut idx = 0;
    while idx < buf.len() {
        match &buf[idx] {
            TokenTree::Ident(ident) => {
                // Get Separator "with/except/at"
                match ident.to_string().as_str() {
                    "with" => {
                        flag_with = true;
                        idx += 1;
                        continue;
                    }
                    "except" => {
                        flag_except = true;
                        idx += 1;
                        continue;
                    }
                    "at" => {
                        flag_at = true;
                        idx += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            TokenTree::Group(group) => {
                if !flag_with && !flag_except && !flag_at {
                    // The tuple length is obtained by calculating the number of parenthesis layers of ((0 + 1) + 1).
                    // Actually the '0' also has a parenthesis wrapped around it, So here have to -1.
                    len = group_num(group.stream()) - 1;
                    idx += 1;
                    continue;
                }
                if flag_with && flag_except && flag_at {
                    // Get the except_index.
                    except_index = group.stream().into_iter().count();
                    idx += 1;
                    continue;
                }
            }
            _ => {}
        }

        // Get TupleParser's 'default' or 'except'
        if flag_with && !flag_at {
            let default_or_except = TokenStream::from((buf[idx]).to_owned());
            if flag_except {
                except.extend(default_or_except);
            } else {
                default.extend(default_or_except);
            }
        }

        idx += 1;
    }
    TupleParser {
        len,
        default,
        except,
        except_index,
    }
}

/// Recursively queried how many layers of [`TokenTree::Group`]
fn group_num(inner: TokenStream) -> usize {
    match inner.into_iter().next() {
        Some(TokenTree::Group(group)) => group_num(group.stream()) + 1,
        _ => 0,
    }
}

/// Necessary data for building tuples
pub(crate) struct TupleParser {
    /// Length of Tuple
    pub(crate) len: usize,
    /// Default value of Tuple
    pub(crate) default: TokenStream,
    /// Except value of Tuple
    pub(crate) except: TokenStream,
    /// Index of the default value in the tuple
    pub(crate) except_index: usize,
}
