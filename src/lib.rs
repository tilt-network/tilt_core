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
            unsafe {
                let _ = Vec::from_raw_parts(ptr, len, len);
            }
        }

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
        // #[unsafe(no_mangle)]
        // pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
        //     let input_slice = unsafe { std::slice::from_raw_parts(ptr, len) };

        //     // Chamar a função com bytes brutos
        //     let mut output = #fn_name(input_slice);

        //     output.shrink_to_fit();

        //     // Retornar bytes brutos diretamente
        //     let out_len = output.len();
        //     let out_ptr = output.as_ptr() as *mut u8;
        //     std::mem::forget(output);

        //     unsafe {
        //         *retptr.offset(0) = out_ptr as u32;
        //         *retptr.offset(1) = out_len as u32;
        //     }
        // }
    //     #[unsafe(no_mangle)]
    //     pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
    //         let input_slice = unsafe { std::slice::from_raw_parts(ptr, len) };

    //         if let Ok(input_str) = std::str::from_utf8(input_slice) {
    //             if let Ok(input): _ = serde_json::from_str(input_str) {
    //                 let output = #fn_name(input);

    //                 let output_json = serde_json::to_string(&output).expect("Failed to serialize");
    //                 let out_bytes = output_json.into_bytes();
    //                 let out_bytes = out_bytes.as_slice().to_vec();
    //                 let out_len = out_bytes.len();
    //                 let out_ptr = out_bytes.as_ptr() as *mut u8;
    //                 std::mem::forget(out_bytes);

    //                 return unsafe {
    //                     *retptr.offset(0) = out_ptr as u32;
    //                     *retptr.offset(1) = out_len as u32;
    //                 }
    //             }
    //         }

    //         let mut output = #fn_name(input_slice);

    //         output.shrink_to_fit();
    //         let out_len = output.len();
    //         let out_ptr = output.as_ptr() as *mut u8;
    //         std::mem::forget(output);

    //         unsafe {
    //             *retptr.offset(0) = out_ptr as u32;
    //             *retptr.offset(1) = out_len as u32;
    //         }
    //     }
    // };

    TokenStream::from(expanded)
}
