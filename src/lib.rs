extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{ParseStream, Result};
use syn::{ItemFn, LitBool, parse::Parse, parse_macro_input};

struct MacroArgs {
    binary: Option<bool>, // None = auto (try JSON first), Some(true) = force binary, Some(false) = force JSON
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

    let execute_impl = match args.binary {
        Some(true) => quote! {
            #[unsafe(no_mangle)]
            pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
                let input_slice = unsafe { std::slice::from_raw_parts(ptr, len) };

                let mut output = #fn_name(input_slice);

                output.shrink_to_fit();
                let out_len = output.len();
                let out_ptr = output.as_ptr() as *mut u8;
                std::mem::forget(output);

                unsafe {
                    *retptr.offset(0) = out_ptr as u32;
                    *retptr.offset(1) = out_len as u32;
                }
            }
        },

        Some(false) => quote! {
            #[unsafe(no_mangle)]
            pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
                let input_slice = unsafe { std::slice::from_raw_parts(ptr, len) };
                let input_str = std::str::from_utf8(input_slice).expect("Invalid UTF-8");

                let input: _ = serde_json::from_str(input_str).expect("Invalid JSON");

                let output = #fn_name(input);

                let output_json = serde_json::to_string(&output).expect("Failed to serialize");
                let out_bytes = output_json.into_bytes();
                let out_bytes = out_bytes.as_slice().to_vec();
                let out_len = out_bytes.len();
                let out_ptr = out_bytes.as_ptr() as *mut u8;
                std::mem::forget(out_bytes);

                unsafe {
                    *retptr.offset(0) = out_ptr as u32;
                    *retptr.offset(1) = out_len as u32;
                }
            }
        },

        None => quote! {
            #[unsafe(no_mangle)]
            pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
                let input_slice = unsafe { std::slice::from_raw_parts(ptr, len) };

                let json_result = std::str::from_utf8(input_slice)
                    .ok()
                    .and_then(|input_str| {
                        serde_json::from_str(input_str).ok()
                    });

                let output = if let Some(json_input) = json_result {
                    #fn_name(json_input)
                } else {
                    #fn_name(input_slice)
                };

                let json_output = serde_json::to_string(&output);

                if let Ok(json) = json_output {
                    let out_bytes = json.into_bytes();
                    let out_len = out_bytes.len();
                    let out_ptr = out_bytes.as_ptr() as *mut u8;
                    std::mem::forget(out_bytes);

                    unsafe {
                        *retptr.offset(0) = out_ptr as u32;
                        *retptr.offset(1) = out_len as u32;
                    }
                } else {
                    let mut out_bytes: Vec<u8> = match std::any::Any::type_id(&output) {
                        id if id == std::any::TypeId::of::<Vec<u8>>() => {
                            let out = unsafe { std::mem::transmute_copy(&output) };
                            std::mem::forget(output);
                            out
                        },
                        _ => {
                            format!("{:?}", output).into_bytes()
                        }
                    };

                    out_bytes.shrink_to_fit();
                    let out_len = out_bytes.len();
                    let out_ptr = out_bytes.as_ptr() as *mut u8;
                    std::mem::forget(out_bytes);

                    unsafe {
                        *retptr.offset(0) = out_ptr as u32;
                        *retptr.offset(1) = out_len as u32;
                    }
                }
            }
        },
    };

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
            unsafe {
                let _ = Vec::from_raw_parts(ptr, len, len);
            }
        }

        #execute_impl
    };

    TokenStream::from(expanded)
}
