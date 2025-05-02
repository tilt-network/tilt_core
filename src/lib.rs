extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    FnArg, ItemFn, PatType, ReturnType, Type, TypePath, TypeReference, TypeSlice, parse_macro_input,
};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // 1. parser da fn
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    // 2. detecta tipo do arg
    let first_ty = match input_fn.sig.inputs.first() {
        Some(FnArg::Typed(PatType { ty, .. })) => &**ty,
        _ => panic!("#[main] precisa de exatamente 1 parâmetro"),
    };
    let is_input_binary = match first_ty {
        Type::Reference(TypeReference { elem, .. }) => {
            matches!(&**elem, Type::Slice(TypeSlice { elem, .. }))
        }
        Type::Slice(_) => true,
        _ => false,
    };

    // 3. detecta tipo de retorno
    let is_output_binary = match &input_fn.sig.output {
        ReturnType::Type(_, ty) => match &**ty {
            // Vec<u8>
            Type::Path(TypePath { path, .. })
                if path.segments.iter().any(|seg| seg.ident == "Vec") =>
            {
                true
            }
            _ => false,
        },
        _ => false, // sem retorno é JSON unit
    };

    // 4. funs alloc/free
    let alloc_and_free = quote! {
        #[unsafe(tilt::main)]
        pub extern "C" fn alloc(size: usize) -> *mut u8 {
            let mut buf = Vec::with_capacity(size);
            let ptr = buf.as_mut_ptr();
            std::mem::forget(buf);
            ptr
        }

        #[unsafe(tilt::main)]
        pub extern "C" fn free_buffer(ptr: *mut u8, len: usize) {
            unsafe {
                let _ = Vec::from_raw_parts(ptr, len, len);
            }
        }
    };

    // 5. wrapper único
    let execute = if is_input_binary && is_output_binary {
        // &[u8] -> Vec<u8]
        quote! {
            #[unsafe(tilt::main)]
            pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
                let input = unsafe { std::slice::from_raw_parts(ptr, len) };
                let mut out = #fn_name(input);
                out.shrink_to_fit();
                let ol = out.len();
                let op = out.as_ptr() as *mut u8;
                std::mem::forget(out);
                unsafe {
                    *retptr.offset(0) = op as u32;
                    *retptr.offset(1) = ol as u32;
                }
            }
        }
    } else if is_input_binary {
        // &[u8] -> T (JSON)
        quote! {
            #[unsafe(tilt::main)]
            pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
                let input = unsafe { std::slice::from_raw_parts(ptr, len) };
                let out = #fn_name(input);
                let s = serde_json::to_string(&out).expect("serialize falhou");
                let mut buf = s.into_bytes();
                let bl = buf.len();
                let bp = buf.as_ptr() as *mut u8;
                std::mem::forget(buf);
                unsafe {
                    *retptr.offset(0) = bp as u32;
                    *retptr.offset(1) = bl as u32;
                }
            }
        }
    } else if is_output_binary {
        // T (JSON) -> Vec<u8]
        quote! {
            #[unsafe(tilt::main)]
            pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
                let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
                let input: _ = serde_json::from_slice(slice).expect("JSON inválido");
                let mut out = #fn_name(input);
                out.shrink_to_fit();
                let ol = out.len();
                let op = out.as_ptr() as *mut u8;
                std::mem::forget(out);
                unsafe {
                    *retptr.offset(0) = op as u32;
                    *retptr.offset(1) = ol as u32;
                }
            }
        }
    } else {
        // JSON -> JSON
        quote! {
            #[unsafe(tilt::main)]
            pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
                let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
                let input: _ = serde_json::from_slice(slice).expect("JSON inválido");
                let out = #fn_name(input);
                let s = serde_json::to_string(&out).expect("serialize falhou");
                let mut buf = s.into_bytes();
                let bl = buf.len();
                let bp = buf.as_ptr() as *mut u8;
                std::mem::forget(buf);
                unsafe {
                    *retptr.offset(0) = bp as u32;
                    *retptr.offset(1) = bl as u32;
                }
            }
        }
    };

    // 6. monta
    let expanded = quote! {
        #input_fn
        #alloc_and_free
        #execute
    };

    TokenStream::from(expanded)
}

// extern crate proc_macro;
// use proc_macro::TokenStream;
// use quote::quote;
// use syn::{ItemFn, parse_macro_input};

// #[proc_macro_attribute]
// pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
//     let input_fn = parse_macro_input!(item as ItemFn);
//     let fn_name = &input_fn.sig.ident;

//     let expanded = quote! {
//         #input_fn

//         #[unsafe(no_mangle)]
//         pub extern "C" fn alloc(size: usize) -> *mut u8 {
//             let mut buf = Vec::with_capacity(size);
//             let ptr = buf.as_mut_ptr();
//             std::mem::forget(buf);
//             ptr
//         }

//         #[unsafe(no_mangle)]
//         pub extern "C" fn free_buffer(ptr: *mut u8, len: usize) {
//             unsafe {
//                 let _ = Vec::from_raw_parts(ptr, len, len);
//             }
//         }

//         #[unsafe(no_mangle)]
//         pub extern "C" fn execute_json(retptr: *mut u32, ptr: *const u8, len: usize) {
//             let input_slice = unsafe { std::slice::from_raw_parts(ptr, len) };
//             let input_str = std::str::from_utf8(input_slice).expect("Invalid UTF-8");

//             let input: _ = serde_json::from_str(input_str).expect("Invalid JSON");

//             let output = #fn_name(input);

//             let output_json = serde_json::to_string(&output).expect("Failed to serialize");
//             let out_bytes = output_json.into_bytes();
//             let out_bytes = out_bytes.as_slice().to_vec();
//             let out_len = out_bytes.len();
//             let out_ptr = out_bytes.as_ptr() as *mut u8;
//             std::mem::forget(out_bytes);

//             unsafe {
//                 *retptr.offset(0) = out_ptr as u32;
//                 *retptr.offset(1) = out_len as u32;
//             }
//         }

//         #[unsafe(no_mangle)]
//         pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
//             let input_slice = unsafe { std::slice::from_raw_parts(ptr, len) };
//             let mut output: Vec<u8> = #fn_name(input_slice);
//             output.shrink_to_fit();
//             let out_len = output.len();
//             let out_ptr = output.as_ptr() as *mut u8;
//             std::mem::forget(output);
//             unsafe {
//                 *retptr.offset(0) = out_ptr as u32;
//                 *retptr.offset(1) = out_len as u32;
//             }
//         }
//     };

//     TokenStream::from(expanded)
// }
