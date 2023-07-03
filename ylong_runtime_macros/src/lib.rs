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

#![allow(clippy::needless_doctest_main)]
#![doc(test(no_crate_inject,))]
//! Macros for use with ylong_runtime
mod entry;
mod select;

use proc_macro::{Delimiter, Group, Punct, Spacing, TokenStream, TokenTree};

/// Implementation detail of the `select!` macro. This macro is **not** intended
/// to be used as part of the public API.
/// # Examples
///
/// ```
/// #[derive(PartialEq, Debug)]
/// enum Out {
///     Finish,
///     Fail,
/// }
/// let tuple = ylong_runtime_macros::tuple_form!(( (((0)+1)+1) ) with Out::Fail except Out::Finish at ( ) );
/// assert_eq!(tuple, (Out::Finish, Out::Fail));
/// ```
#[proc_macro]
#[doc(hidden)]
pub fn tuple_form(input: TokenStream) -> TokenStream {
    let tuple_parser = select::tuple_parser(input);

    let mut group_inner = TokenStream::new();

    // Constructing Tuples
    for i in 0..tuple_parser.len {
        if i == tuple_parser.except_index {
            // Set 'except_index' at index
            group_inner.extend(tuple_parser.except.clone());
        } else {
            // Set 'default'
            group_inner.extend(tuple_parser.default.clone());
        }
        // Add ',' separator
        if i != tuple_parser.len - 1 {
            let punct: Punct = Punct::new(',', Spacing::Alone);
            group_inner.extend(TokenStream::from(TokenTree::from(punct)));
        }
    }
    // Add parentheses on the outermost
    let tuple = Group::new(Delimiter::Parenthesis, group_inner);

    TokenStream::from(TokenTree::from(tuple))
}

/// Mark the async fn to execute with the selected runtime.
///
/// You can set the runtime by adding attributes after the main ident in a parentheses. Attributes that can be set include runtime flavor , ....
///
/// Note: This procedural macro is just to simplify the setup of ylong runtime. If you need more complex and specific settings. You should directly invoke the runtime builder function.
///
/// Note: Please make sure that the corresponding feature is enabled, otherwise it will show that the corresponding function cannot be found.
///
/// ## Usage
///
/// ### Current runtime
///
/// ```no run
/// #[ylong_runtime::main(flavor = "current")]
/// async fn main() {
///     println!("hello");
/// }
/// ```
///
/// Expanded Code
///
/// ```no run
/// fn main() {
///     let body = async {
///         println!("hello");
///     };
///     let rt = ylong_runtime::builder::RuntimeBuilder::new_current_thread().build().unwrap();
///     rt.block_on(body)
/// }
/// ```
///
/// ### Global runtime
///
/// ```no run
/// #[ylong_runtime::main]
/// async fn main() {
///     println!("hello");
/// }
/// ```
///
/// Expanded Code
///
/// ```no run
/// fn main() {
///     let body = async {
///         println!("hello");
///     };
///     ylong_runtime::block_on(body);
/// }
/// ```
#[proc_macro_attribute]
pub fn main(args: TokenStream, input: TokenStream) -> TokenStream {
    entry::main(args, input)
}

/// ##Usage
///
/// ```no run
/// #[ylong_runtime::test]
/// async fn test() {
///     assert_eq!(1, 1);
/// }
/// ```
///
/// Expanded Code
///
/// ```no run
/// #[test]
/// fn test() {
///     let mut block = async {
///         assert_eq!(1, 1);
///     };
///     let body = unsafe {
///         std::pin::Pin::<&mut dyn std::future::Future<Output = ()>>::new_unchecked(&mut body)
///     };
///     ylong_runtime::block_on(body);
/// }
/// ```
#[proc_macro_attribute]
pub fn test(args: TokenStream, input: TokenStream) -> TokenStream {
    entry::test(args, input)
}
