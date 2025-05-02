extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::{ItemFn, LitBool, parse_macro_input};

struct MacroArgs {
    binary: Option<bool>,
}
impl Parse for MacroArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.is_empty() {
            return Ok(MacroArgs { binary: None });
        }
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Ident) {
            let ident: syn::Ident = input.parse()?;
            if ident == "binary" {
                if input.is_empty() || input.peek(syn::token::Comma) {
                    return Ok(MacroArgs { binary: Some(true) });
                }
                if input.peek(syn::token::Eq) {
                    let _: syn::token::Eq = input.parse()?;
                    let value: LitBool = input.parse()?;
                    return Ok(MacroArgs {
                        binary: Some(value.value),
                    });
                }
            }
        }
        Ok(MacroArgs { binary: None })
    }
}

#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as MacroArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let json_impl = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
            let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
            if let Ok(s) = std::str::from_utf8(slice) {
                if let Ok(json_in) = serde_json::from_str::<_>(s) {
                    let mut out = serde_json::to_string(&#fn_name(json_in))
                        .expect("serialize failed").into_bytes();
                    out.shrink_to_fit();
                    unsafe {
                        *retptr.offset(0) = out.as_ptr() as u32;
                        *retptr.offset(1) = out.len() as u32;
                    }
                    return;
                }
            }
            let mut out = #fn_name(slice);
            out.shrink_to_fit();
            unsafe {
                *retptr.offset(0) = out.as_ptr() as u32;
                *retptr.offset(1) = out.len() as u32;
            }
        }
    };

    let pure_json = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
            let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
            let s = std::str::from_utf8(slice).expect("Invalid UTF8");
            let input: _ = serde_json::from_str(s).expect("Invalid JSON");
            let mut out = serde_json::to_string(&#fn_name(input))
                .expect("serialize failed").into_bytes();
            out.shrink_to_fit();
            unsafe {
                *retptr.offset(0) = out.as_ptr() as u32;
                *retptr.offset(1) = out.len() as u32;
            }
        }
    };

    let pure_bin = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
            let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
            let mut out = #fn_name(slice);
            out.shrink_to_fit();
            unsafe {
                *retptr.offset(0) = out.as_ptr() as u32;
                *retptr.offset(1) = out.len() as u32;
            }
        }
    };

    let execute_impl = match args.binary {
        Some(false) => pure_json,
        Some(true) => pure_bin,
        None => json_impl,
    };

    let expanded = quote! {
        #input_fn
        #[unsafe(no_mangle)] pub extern "C" fn alloc(size: usize) -> *mut u8 {
            let mut v = Vec::with_capacity(size);
            let ptr = v.as_mut_ptr();
            std::mem::forget(v);
            ptr
        }
        #[unsafe(no_mangle)] pub extern "C" fn free_buffer(ptr: *mut u8, len: usize) {
            unsafe { let _ = Vec::from_raw_parts(ptr, len, len); }
        }
        #execute_impl
    };
    TokenStream::from(expanded)
}
