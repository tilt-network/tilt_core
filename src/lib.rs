extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, PatType, ReturnType, Type, TypePath, TypeReference, parse_macro_input};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // 1. parse da função alvo
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    // 2. detecta se a entrada é binária (&[u8] ou slice de u8)
    let first_ty = match input_fn.sig.inputs.first() {
        Some(FnArg::Typed(PatType { ty, .. })) => &**ty,
        _ => panic!("#[main] precisa de 1 argumento"),
    };

    let is_input_binary = match first_ty {
        // direta slice: &[u8]
        Type::Slice(_) => true,
        // referência a slice: & [u8]
        Type::Reference(TypeReference { elem, .. }) => matches!(&**elem, Type::Slice(_)),
        _ => false,
    };

    // 3. detecta se o retorno é Vec<u8>
    let is_output_binary = match &input_fn.sig.output {
        ReturnType::Type(_, ty) => matches!(
            &**ty,
            Type::Path(TypePath { path, .. })
                if path.segments.iter().any(|seg| seg.ident == "Vec")
        ),
        _ => false,
    };

    // 4. cria um módulo único para não colidir nomes
    let mod_name = format_ident!("__tilt_exports_{}", fn_name);

    // 5. gera alloc/free (sempre)
    let alloc_free = quote! {
        #[unsafe(tilt::main)]
        pub extern "C" fn alloc(size: usize) -> *mut u8 {
            let mut buf = Vec::with_capacity(size);
            let ptr = buf.as_mut_ptr();
            std::mem::forget(buf);
            ptr
        }
        #[unsafe(tilt::main)]
        pub extern "C" fn free_buffer(ptr: *mut u8, len: usize) {
            unsafe { let _ = Vec::from_raw_parts(ptr, len, len); }
        }
    };

    // 6. gera só um execute com os 4 cenários combinados
    let execute = if is_input_binary && is_output_binary {
        // &[u8] -> Vec<u8>
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
                    *retptr.add(0) = op as u32;
                    *retptr.add(1) = ol as u32;
                }
            }
        }
    } else if is_input_binary {
        // &[u8] -> JSON
        quote! {
            #[unsafe(tilt::main)]
            pub extern "C" fn execute(retptr: *mut u32, ptr: *const u8, len: usize) {
                let input = unsafe { std::slice::from_raw_parts(ptr, len) };
                let out = #fn_name(input);
                let mut buf = serde_json::to_vec(&out).expect("serialize falhou");
                let bl = buf.len();
                let bp = buf.as_ptr() as *mut u8;
                std::mem::forget(buf);
                unsafe {
                    *retptr.add(0) = bp as u32;
                    *retptr.add(1) = bl as u32;
                }
            }
        }
    } else if is_output_binary {
        // JSON -> Vec<u8>
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
                    *retptr.add(0) = op as u32;
                    *retptr.add(1) = ol as u32;
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
                let mut buf = serde_json::to_vec(&out).expect("serialize falhou");
                let bl = buf.len();
                let bp = buf.as_ptr() as *mut u8;
                std::mem::forget(buf);
                unsafe {
                    *retptr.add(0) = bp as u32;
                    *retptr.add(1) = bl as u32;
                }
            }
        }
    };

    // 7. monta tudo dentro do módulo e dá pub use
    let expanded = quote! {
        #input_fn

        // módulo gerado
        #[doc(hidden)]
        mod #mod_name {
            use super::*;
            #alloc_free
            #execute
        }

        // traz as funções pro escopo raiz só uma vez
        pub use #mod_name::*;
    };

    expanded.into()
}
