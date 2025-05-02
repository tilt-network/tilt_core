extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

/// Atributo `#[main]` gera executável que aceita JSON ou binário automaticamente.
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
            unsafe {
                let _ = Vec::from_raw_parts(ptr, len, len);
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
            let input = unsafe { std::slice::from_raw_parts(ptr, len) };

            // JSON path: se for UTF-8 válido e parse JSON bem-sucedido
            if let Ok(s) = std::str::from_utf8(input) {
                if let Ok(val) = serde_json::from_str::<_>(s) {
                    // chama função com struct/obj
                    let output = #fn_name(val);
                    // serializa pra JSON bytes
                    let mut bytes = serde_json::to_vec(&output).expect("serialize failed");
                    bytes.shrink_to_fit();
                    unsafe {
                        *retptr.offset(0) = bytes.as_ptr() as u32;
                        *retptr.offset(1) = bytes.len() as u32;
                    }
                    std::mem::forget(bytes);
                    return;
                }
            }

            let mut bytes = #fn_name(input);
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

// #[unsafe(no_mangle)]
// pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
//     let input_slice = unsafe { std::slice::from_raw_parts(ptr, len) };
//     let mut output: Vec<u8> = #fn_name(input_slice);
//     output.shrink_to_fit();
//     let out_len = output.len();
//     let out_ptr = output.as_ptr() as *mut u8;
//     std::mem::forget(output);
//     unsafe {
//         *retptr.offset(0) = out_ptr as u32;
//         *retptr.offset(1) = out_len as u32;
//     }
// }
