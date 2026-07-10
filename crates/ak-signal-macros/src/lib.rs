use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parser, punctuated::Punctuated, FnArg, ItemFn, Meta, Token};

#[proc_macro_attribute]
pub fn signal(attr: TokenStream, item: TokenStream) -> TokenStream {
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let metas = parser.parse(attr).unwrap_or_default();

    let mut name = String::new();
    let mut template = String::new();
    for m in metas {
        if let Meta::NameValue(nv) = m {
            if let Some(ident) = nv.path.get_ident() {
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(v), .. }) = &nv.value {
                    match ident.to_string().as_str() {
                        "name" => name = v.value(),
                        "template" => template = v.value(),
                        _ => {}
                    }
                }
            }
        }
    }

    let f: ItemFn = syn::parse(item).expect("invalid function");
    let fn_name = &f.sig.ident;
    let vis = &f.vis;
    let sig = &f.sig;
    let block = &f.block;

    let name_str = if name.is_empty() {
        fn_name.to_string()
    } else {
        name.clone()
    };
    let template_str = if template.is_empty() {
        format!("{{freq}}_{}", name_str)
    } else {
        template
    };

    let gen = quote! {
        #vis #sig #block

        #[::inventory::ctr::submit]
        static __AK_SIGNAL_#fn_name: ::ak_signals::types::AkSignalMeta = ::ak_signals::types::AkSignalMeta {
            name: #name_str,
            template: #template_str,
            func: ::ak_signals::types::AkSignalFnRef::Closure(Box::new(#fn_name)),
        };
    };

    gen.into()
}
