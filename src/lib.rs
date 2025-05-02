extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{ParseStream, Result};
use syn::{ItemFn, LitBool, parse::Parse, parse_macro_input};

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
                // `#[main(binary)]`
                if input.is_empty() || input.peek(syn::token::Comma) {
                    return Ok(MacroArgs { binary: Some(true) });
                }
                // `#[main(binary = false)]`
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

    // Implementação binária (bytes brutos).
    let binary_impl = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
            let input_slice = unsafe { std::slice::from_raw_parts(ptr, len) };
            let mut output: Vec<u8> = #fn_name(input_slice);
            output.shrink_to_fit();
            let out_len = output.len();
            let out_ptr = output.as_ptr() as *mut u8;
            std::mem::forget(output);
            unsafe {
                *retptr.offset(0) = out_ptr as u32;
                *retptr.offset(1) = out_len as u32;
            }
        }
    };

    // Implementação JSON (entrada e saída como JSON).
    let json_impl = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
            let input_slice = unsafe { std::slice::from_raw_parts(ptr, len) };
            let input_str = std::str::from_utf8(input_slice).expect("Invalid UTF-8");
            let input: _ = serde_json::from_str(input_str).expect("Invalid JSON");
            let mut out_bytes = serde_json::to_string(&#fn_name(input))
                .expect("Failed to serialize").into_bytes();
            out_bytes.shrink_to_fit();
            let out_len = out_bytes.len();
            let out_ptr = out_bytes.as_ptr() as *mut u8;
            std::mem::forget(out_bytes);
            unsafe {
                *retptr.offset(0) = out_ptr as u32;
                *retptr.offset(1) = out_len as u32;
            }
        }
    };

    // Decide qual usar: `binary = false` → JSON; caso contrário, binário.
    let execute_impl = match args.binary {
        Some(false) => json_impl,
        _ => binary_impl,
    };

    // Expande o código final com alloc, free_buffer e execute.
    let expanded = quote! {
        #input_fn

        #[unsafe(no_mangle)]
        pub extern "C" fn alloc(size: usize) -> *mut u8 {
            let mut buf = Vec::with_capacity(size);
            let ptr = buf.as_mut_ptr();
            std::mem::forget(buf);
            ptr
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn free_buffer(ptr: *mut u8, len: usize) {
            unsafe { let _ = Vec::from_raw_parts(ptr, len, len); }
        }

        #execute_impl
    };

    TokenStream::from(expanded)
}
