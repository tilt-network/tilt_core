extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::{FnArg, ItemFn, PatType};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let arg_ty = input_fn
        .sig
        .inputs
        .iter()
        .find_map(|arg| {
            if let FnArg::Typed(PatType { ty, .. }) = arg {
                Some(&**ty)
            } else {
                None
            }
        })
        .expect("Function must have one argument");

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
            if let Ok(val) = serde_json::from_slice::<#arg_ty>(data) {
                let mut bytes = serde_json::to_vec(&#fn_name(val)).expect("serialize failed");
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
