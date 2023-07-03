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

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse::Parser, punctuated::Punctuated, spanned::Spanned, token::Comma, Error, ItemFn,
    MetaNameValue, Result,
};

fn stream_with_error(mut stream: TokenStream2, errors: Option<Error>) -> TokenStream2 {
    if let Some(error) = errors {
        stream.extend(error.into_compile_error());
    }
    stream
}

pub fn main(args: TokenStream, item: TokenStream) -> TokenStream {
    intermediate(TokenStream2::from(args), TokenStream2::from(item), false).into()
}

pub fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    let head = quote! {
        #[::core::prelude::v1::test]
    };
    let body = intermediate(TokenStream2::from(args), TokenStream2::from(item), true);
    let result = quote! {
        #head
        #body
    };
    result.into()
}

fn intermediate(args: TokenStream2, item: TokenStream2, is_test: bool) -> TokenStream2 {
    let async_fn = match syn::parse2::<ItemFn>(item) {
        Ok(async_fn) => async_fn,
        Err(e) => return e.into_compile_error(),
    };
    let mut errors = None;

    if !is_test && async_fn.sig.ident != "main" {
        let e = Error::new_spanned(
            &async_fn.sig.ident,
            "Currently the main procedural macro only accepts main function",
        );
        errors = Some(e);
    }

    if async_fn.sig.asyncness.is_none() {
        let e = Error::new_spanned(&async_fn.sig, "Missing `async` keyword");
        errors = combine_errors(errors, e);
    }

    let config = build_config(args, is_test);

    let stream = match config {
        Ok(config) => build_stream(async_fn, config),
        Err(e) => {
            errors = combine_errors(errors, e);
            build_stream(async_fn, Config::new(is_test))
        }
    };

    stream_with_error(stream, errors)
}

fn combine_errors(mut errors: Option<Error>, e: Error) -> Option<Error> {
    if let Some(error) = errors.as_mut() {
        error.combine(e);
    } else {
        errors = Some(e);
    }
    errors
}

#[derive(PartialEq)]
enum RuntimeFlavor {
    Global,
    Current,
}
#[derive(PartialEq)]
struct Config {
    flavor: Option<RuntimeFlavor>,
    is_test: bool,
}

impl Config {
    fn new(is_test: bool) -> Self {
        Config {
            flavor: None,
            is_test,
        }
    }

    fn set_flavor(&mut self, lit_str: String, span: Span) -> Result<()> {
        if self.flavor.is_some() {
            return Err(Error::new(span, "`flavor` is set multi times"));
        }

        match lit_str.as_str() {
            "current" => {
                self.flavor = Some(RuntimeFlavor::Current);
            }
            "global" => {
                self.flavor = Some(RuntimeFlavor::Global);
            }
            name => {
                return Err(Error::new(
                    span,
                    format!("Invalid flavor value `{name}`, expected one of `current`,`global`",),
                ));
            }
        }
        Ok(())
    }
    fn set_default(&mut self) {
        if self.flavor.is_none() {
            self.flavor = Some(RuntimeFlavor::Global);
        }
    }
}

fn lit_parse_str(lit: syn::Lit, span: Span, ident: &str) -> Result<String> {
    if let syn::Lit::Str(lit_str) = lit {
        Ok(lit_str.value())
    } else {
        Err(Error::new(
            span,
            format!("The value of {ident} must be a literal"),
        ))
    }
}

fn build_config(args: TokenStream2, is_test: bool) -> Result<Config> {
    let mut config = Config::new(is_test);

    let args = Punctuated::<MetaNameValue, Comma>::parse_terminated.parse2(args)?;

    for arg in args {
        let ident = arg
            .path
            .get_ident()
            .ok_or(Error::new(arg.path.span(), "Invalid ident"))?;

        let ident_string = ident.to_string();
        let lit = arg.lit;

        let span = lit.span();

        let lit_str = lit_parse_str(lit, span, ident_string.as_str())?;
        match ident_string.as_str() {
            "flavor" => config.set_flavor(lit_str, span)?,
            name => {
                return Err(Error::new(
                    arg.path.span(),
                    format!("Invalid Attribute `{name}`, supported attributes `flavor` ",),
                ))
            }
        }
    }
    Ok(config)
}

fn build_stream(mut async_fn: ItemFn, mut config: Config) -> TokenStream2 {
    async_fn.sig.asyncness = None;
    config.set_default();
    let block = async_fn.block;
    let body = match config.is_test {
        true => {
            let output = match &async_fn.sig.output {
                syn::ReturnType::Default => quote! {()},
                syn::ReturnType::Type(_, output_type) => quote! {#output_type},
            };
            quote! {
                let mut block = async #block;
                let body = unsafe {
                   std::pin::Pin::<&mut dyn std::future::Future<Output = #output>>::new_unchecked(&mut block)
               };
            }
        }
        false => quote! {
            let body = async #block;
        },
    };

    let new_block = match config.flavor.unwrap() {
        RuntimeFlavor::Current => quote! {
            {
                #body
                let rt = ylong_runtime::builder::RuntimeBuilder::new_current_thread().build().unwrap();
                rt.block_on(body)
            }
        },
        RuntimeFlavor::Global => quote! {
            {
                #body
                ylong_runtime::block_on(body)
            }
        },
    };
    async_fn.block = syn::parse2(new_block).expect("parse failure");
    quote! {
        #async_fn
    }
}

#[cfg(test)]
mod test {
    use super::*;
    /// UT test cases for `build_stream` in main procedural macro.
    ///
    /// # Brief
    /// 1. Creates an `async_fn` using `quote!`.
    /// 2. Creates an `output` by calling `build_stream` with different `config`.
    /// 3. Checks if the result is correct.
    #[test]
    fn ut_build_stream_main() {
        let item = quote! {
            async fn main() {
                println!("hello");
            }
        };
        let async_fn: ItemFn = syn::parse2(item).unwrap();

        // default
        let args = quote! {};
        let config = build_config(args, false).unwrap();

        let output = build_stream(async_fn.clone(), config);
        let output: ItemFn = syn::parse2(output).unwrap();

        let res = quote! {
            fn main() {
                let body = async {
                    println!("hello");
                };
                ylong_runtime::block_on(body)
            }
        };
        let res: ItemFn = syn::parse2(res).unwrap();
        assert_eq!(res, output);

        //current
        let args = quote! {
            flavor = "current"
        };
        let config = build_config(args, false).unwrap();

        let output = build_stream(async_fn, config);
        let output: ItemFn = syn::parse2(output).unwrap();

        let res = quote! {
            fn main() {
                let body = async {
                    println!("hello");
                };
                let rt = ylong_runtime::builder::RuntimeBuilder::new_current_thread().build().unwrap();
                rt.block_on(body)
            }
        };
        let res: ItemFn = syn::parse2(res).unwrap();
        assert_eq!(res, output);
    }

    /// UT test cases for `build_stream` in test procedural macro.
    ///
    /// # Brief
    /// 1. Creates an `async_fn` using `quote!`.
    /// 2. Creates an `output` by calling `build_stream` with different `config`.
    /// 3. Checks if the result is correct.
    #[test]
    fn ut_build_stream_test() {
        let item = quote! {
            async fn test() {
                assert_eq!(1, 1);
            }
        };
        let async_fn: ItemFn = syn::parse2(item).unwrap();

        //default config for test procedural macro, where the flavor is `current`.
        let args = quote! {};
        let config = build_config(args, true).unwrap();

        let output = build_stream(async_fn.clone(), config);
        let output: ItemFn = syn::parse2(output).unwrap();

        let res = quote! {
            fn test() {
                let mut block = async{
                    assert_eq!(1, 1);
                };
                let body = unsafe{
                    std::pin::Pin::<&mut dyn std::future::Future<Output = ()>>::new_unchecked(&mut block)
                };
                ylong_runtime::block_on(body)
            }
        };
        let res: ItemFn = syn::parse2(res).unwrap();
        assert_eq!(res, output);

        //current

        let args = quote! {
            flavor = "current"
        };
        let config = build_config(args, true).unwrap();

        let output = build_stream(async_fn, config);
        let output: ItemFn = syn::parse2(output).unwrap();

        let res = quote! {
            fn test() {
                let mut block = async{
                    assert_eq!(1, 1);
                };
                let body = unsafe{
                    std::pin::Pin::<&mut dyn std::future::Future<Output = ()>>::new_unchecked(&mut block)
                };
                let rt = ylong_runtime::builder::RuntimeBuilder::new_current_thread().build().unwrap();
                rt.block_on(body)
            }
        };
        let res: ItemFn = syn::parse2(res).unwrap();
        assert_eq!(res, output);
    }
}
