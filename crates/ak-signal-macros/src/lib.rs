use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, ExprLit, ItemFn, Lit, Meta, Token};

/// `#[signal(category = "kline", name = "...", template = "...", opcode = "...", param_kind = "...")]`
///
/// 用法:
/// ```ignore
/// #[signal(
///     category = "kline",
///     name = "ak_ttm_squeeze_V260711",
///     template = "{freq}_D{di}TTM#{n1}#{n2}#{n3}#{n4}#{n5}#{n6}_V260711",
///     opcode = "AkTtmSqueeze",
///     param_kind = "AkTtmSqueeze"
/// )]
/// pub fn ak_ttm_squeeze_v260711(
///     bars: &[BarPoint],
///     params: &HashMap<String, String>,
/// ) -> Vec<SignalOutput> { ... }
/// ```
#[proc_macro_attribute]
pub fn signal(attr: TokenStream, item: TokenStream) -> TokenStream {
    // 解析属性
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let metas = match parser.parse(attr) {
        Ok(m) => m,
        Err(e) => return e.to_compile_error().into(),
    };

    let mut category: Option<String> = None;
    let mut name: Option<String> = None;
    let mut template: Option<String> = None;
    let mut opcode: Option<String> = None;
    let mut param_kind: Option<String> = None;

    for m in metas {
        if let Meta::NameValue(nv) = m {
            if let Some(ident) = nv.path.get_ident() {
                if let Expr::Lit(ExprLit { lit: Lit::Str(v), .. }) = &nv.value {
                    match ident.to_string().as_str() {
                        "category" => category = Some(v.value()),
                        "name" => name = Some(v.value()),
                        "template" => template = Some(v.value()),
                        "opcode" => opcode = Some(v.value()),
                        "param_kind" => param_kind = Some(v.value()),
                        _ => {}
                    }
                }
            }
        }
    }

    // 解析函数定义
    let f: ItemFn = match syn::parse(item) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let mut errors: Vec<proc_macro2::TokenStream> = Vec::new();

    let category = category.unwrap_or_default();
    let name = name.unwrap_or_default();
    let template = template.unwrap_or_default();
    let opcode = opcode.unwrap_or_default();
    let param_kind = param_kind.unwrap_or_default();

    // === 校验 ===
    if category != "kline" && category != "trader" {
        errors.push(quote! { compile_error!("#[signal] category 必须是 kline 或 trader"); });
    }
    if name.is_empty() || template.is_empty() || opcode.is_empty() || param_kind.is_empty() {
        errors.push(quote! { compile_error!("#[signal] name/template/opcode/param_kind 不能为空"); });
    }

    // 函数名版本校验：ak_ttm_squeeze_v260711 -> name must be ak_ttm_squeeze_V260711
    let fn_ident = f.sig.ident.to_string();
    if !fn_ident.contains("_v") {
        errors.push(quote! { compile_error!("#[signal] 函数名必须包含 _v<版本号>（如 ak_xxx_v260711）"); });
    } else {
        let expected = if let Some((head, tail)) = fn_ident.rsplit_once("_v") {
            if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
                format!("{head}_V{tail}")
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        if expected.is_empty() || (!name.is_empty() && expected != name) {
            errors.push(quote! { compile_error!("#[signal] name 必须与函数名版本后缀一致，例如 foo_v230101 -> name = \"foo_V230101\""); });
        }
    }

    // 参数数量校验
    let argc = f.sig.inputs.len();
    if argc != 2 {
        errors.push(quote! { compile_error!("ak_rust 信号函数必须有 2 个参数: (&[BarPoint], &HashMap<String, String>) -> Vec<SignalOutput>"); });
    }

    let vis = &f.vis;
    let sig = &f.sig;
    let block = &f.block;

    // 生成 inventory 注册常量
    let meta_const_ident = syn::Ident::new(
        &format!("__AK_SIGNAL_META_{}", f.sig.ident).to_uppercase(),
        f.sig.ident.span(),
    );
    let fn_name = &f.sig.ident;

    let out = quote! {
        #(#errors)*
        #vis #sig #block

        #[doc(hidden)]
        #[allow(non_upper_case_globals, dead_code)]
        pub const #meta_const_ident: ak_signals::types::SignalDescriptor =
            ak_signals::types::SignalDescriptor {
                category: #category,
                name: #name,
                template: #template,
                opcode: #opcode,
                param_kind: #param_kind,
                func: #fn_name as ak_signals::types::AkSignalFn,
            };

        inventory::submit! {
            #meta_const_ident
        }
    };

    out.into()
}

#[doc(hidden)]
#[proc_macro_attribute]
pub fn signal_module(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // 占位：与 CZSC 签名兼容，当前不做模块级校验
    item
}
