extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

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

        #[unsafe(no_mangle)]
        pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
            let data = unsafe { std::slice::from_raw_parts(ptr, len) };
            if let Ok(json_out) = serde_json::from_slice::<serde_json::Value>(data) {
                let output = #fn_name(json_out);
                let mut bytes = serde_json::to_vec(&output).expect("serialize failed");
                bytes.shrink_to_fit();
                unsafe {
                    *retptr.offset(0) = bytes.as_ptr() as u32;
                    *retptr.offset(1) = bytes.len() as u32;
                }
                std::mem::forget(bytes);
                return;
            }
            let mut bytes = #fn_name(data);
            bytes.shrink_to_fit();
            unsafe {
                *retptr.offset(0) = bytes.as_ptr() as u32;
                *retptr.offset(1) = bytes.len() as u32;
            }
            std::mem::forget(bytes);
        }
    };
    TokenStream::from(expanded)
}
