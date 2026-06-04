//! Procedural macros for the Arvik web framework.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Error, FnArg, Ident, ImplItem, ItemFn, ItemImpl, LitStr, Path, PathArguments, Result, Token,
    Type, parse_macro_input,
};

/// Improve handler type errors by forcing extractor bounds at the function site.
#[proc_macro_attribute]
pub fn debug_handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as DebugHandlerArgs);
    let input = match syn::parse::<ItemFn>(item.clone()) {
        Ok(input) => input,
        Err(error) => return error.to_compile_error().into(),
    };

    expand_debug_handler(args, input).into()
}

/// Attach route metadata to a handler.
#[proc_macro_attribute]
pub fn route(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as RouteArgs);
    expand_route(args.method, args.path, item)
}

/// Attach GET route metadata to a handler.
#[proc_macro_attribute]
pub fn get(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shorthand_route("GET", attr, item)
}

/// Attach POST route metadata to a handler.
#[proc_macro_attribute]
pub fn post(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shorthand_route("POST", attr, item)
}

/// Attach PUT route metadata to a handler.
#[proc_macro_attribute]
pub fn put(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shorthand_route("PUT", attr, item)
}

/// Attach DELETE route metadata to a handler.
#[proc_macro_attribute]
pub fn delete(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shorthand_route("DELETE", attr, item)
}

/// Attach PATCH route metadata to a handler.
#[proc_macro_attribute]
pub fn patch(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shorthand_route("PATCH", attr, item)
}

/// Attach HEAD route metadata to a handler.
#[proc_macro_attribute]
pub fn head(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shorthand_route("HEAD", attr, item)
}

/// Attach OPTIONS route metadata to a handler.
#[proc_macro_attribute]
pub fn options(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shorthand_route("OPTIONS", attr, item)
}

/// Attach TRACE route metadata to a handler.
#[proc_macro_attribute]
pub fn trace(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shorthand_route("TRACE", attr, item)
}

/// Attach catch-all method route metadata to a handler.
#[proc_macro_attribute]
pub fn any(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shorthand_route("ANY", attr, item)
}

/// Collect annotated route handlers into a router registration closure.
#[proc_macro]
pub fn collect_routes(input: TokenStream) -> TokenStream {
    let routes = parse_macro_input!(input as RouteList);
    expand_collect_routes(routes).into()
}

/// Implement `Handler<(Request,), S>` for a struct via an inherent `impl` block.
#[proc_macro_attribute]
pub fn handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return Error::new(Span::call_site(), "#[handler] does not accept arguments")
            .to_compile_error()
            .into();
    }

    let input = match syn::parse::<ItemImpl>(item.clone()) {
        Ok(input) => input,
        Err(error) => return error.to_compile_error().into(),
    };

    expand_handler(input).into()
}

struct DebugHandlerArgs {
    state: Option<Type>,
}

impl Parse for DebugHandlerArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.is_empty() {
            return Ok(Self { state: None });
        }

        let ident: Ident = input.parse()?;
        if ident != "state" {
            return Err(Error::new(ident.span(), "expected `state = Type`"));
        }
        input.parse::<Token![=]>()?;
        let state = input.parse()?;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(Error::new(
                input.span(),
                "unexpected debug_handler argument",
            ));
        }

        Ok(Self { state: Some(state) })
    }
}

struct RouteArgs {
    method: String,
    path: LitStr,
}

impl Parse for RouteArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let method: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let path = input.parse()?;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(Error::new(input.span(), "unexpected route argument"));
        }

        Ok(Self {
            method: method.to_string(),
            path,
        })
    }
}

struct ShorthandRouteArgs {
    path: LitStr,
}

impl Parse for ShorthandRouteArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let path = input.parse()?;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(Error::new(input.span(), "unexpected route argument"));
        }

        Ok(Self { path })
    }
}

struct RouteList {
    paths: Punctuated<Path, Token![,]>,
}

impl Parse for RouteList {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(Self {
            paths: Punctuated::parse_terminated(input)?,
        })
    }
}

fn expand_debug_handler(args: DebugHandlerArgs, input: ItemFn) -> TokenStream2 {
    let arvik = arvik_path();
    let mut errors = Vec::new();
    let sig = &input.sig;
    let fn_ident = &sig.ident;

    if sig.asyncness.is_none() {
        errors.push(Error::new_spanned(
            sig.fn_token,
            "#[debug_handler] requires an async function",
        ));
    }

    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        errors.push(Error::new_spanned(
            &sig.generics,
            "#[debug_handler] does not support generic handler functions",
        ));
    }

    if sig.inputs.len() > 16 {
        errors.push(Error::new_spanned(
            &sig.inputs,
            "Arvik handlers support at most 16 extractors",
        ));
    }

    let mut arg_types = Vec::new();
    for arg in &sig.inputs {
        match arg {
            FnArg::Typed(pat_ty) => arg_types.push((*pat_ty.ty).clone()),
            FnArg::Receiver(receiver) => errors.push(Error::new_spanned(
                receiver,
                "#[debug_handler] can only be used on free functions",
            )),
        }
    }

    let has_state_extractor = arg_types.iter().any(is_state_type);
    if has_state_extractor && args.state.is_none() {
        errors.push(Error::new(
            sig.inputs.span(),
            "handlers using State<T> must declare #[debug_handler(state = AppState)]",
        ));
    }

    let body_positions: Vec<usize> = arg_types
        .iter()
        .enumerate()
        .filter_map(|(idx, ty)| is_builtin_body_type(ty).then_some(idx))
        .collect();

    if body_positions.len() > 1 {
        errors.push(Error::new(
            sig.inputs.span(),
            "handlers can have only one body-consuming extractor",
        ));
    }

    if let Some(last_index) = arg_types.len().checked_sub(1) {
        for idx in body_positions {
            if idx != last_index {
                errors.push(Error::new_spanned(
                    &arg_types[idx],
                    "body-consuming extractors must be the last handler argument",
                ));
            }
        }
    }

    if !errors.is_empty() {
        let error_tokens = errors.into_iter().map(|error| error.to_compile_error());
        return quote! {
            #input
            #(#error_tokens)*
        };
    }

    let state_ty = args
        .state
        .map_or_else(|| quote!(()), |state| quote!(#state));
    let parts_assertions = arg_types
        .iter()
        .take(arg_types.len().saturating_sub(1))
        .map(|ty| {
            quote_spanned! {ty.span()=>
                __arvik_assert_from_request_parts::<#ty, #state_ty>();
            }
        });
    let last_assertion = arg_types.last().map(|ty| {
        quote_spanned! {ty.span()=>
            __arvik_assert_from_request::<#ty, #state_ty, _>();
        }
    });

    quote! {
        #input

        #[allow(non_snake_case, dead_code)]
        const _: () = {
            fn __arvik_debug_handler_checks() {
                fn __arvik_assert_from_request_parts<T, S>()
                where
                    T: #arvik::FromRequestParts<S>,
                {}

                fn __arvik_assert_from_request<T, S, M>()
                where
                    T: #arvik::FromRequest<S, M>,
                {}

                fn __arvik_assert_handler<H, T, S>(_handler: H)
                where
                    H: #arvik::Handler<T, S>,
                {}

                #(#parts_assertions)*
                #last_assertion
                __arvik_assert_handler::<_, _, #state_ty>(#fn_ident);
            }
        };
    }
}

fn expand_shorthand_route(method: &str, attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ShorthandRouteArgs);
    expand_route(method.to_string(), args.path, item)
}

fn expand_route(method: String, path: LitStr, item: TokenStream) -> TokenStream {
    let input = match syn::parse::<ItemFn>(item.clone()) {
        Ok(input) => input,
        Err(error) => return error.to_compile_error().into(),
    };

    let route = match RouteMethod::parse(&method, Span::call_site()) {
        Ok(route) => route,
        Err(error) => {
            return quote! {
                #input
                #error
            }
            .into();
        }
    };

    let normalized_path = match normalize_route_path(&path.value()) {
        Ok(path) => path,
        Err(message) => {
            let error = Error::new(path.span(), message).to_compile_error();
            return quote! {
                #input
                #error
            }
            .into();
        }
    };

    let arvik = arvik_path();
    let vis = &input.vis;
    let fn_ident = &input.sig.ident;
    let helper_ident = route_helper_ident(fn_ident);
    let meta_ident = route_meta_ident(fn_ident);
    let constructor = route.constructor_ident();
    let method_filter = route.method_filter_ident();

    quote! {
        #input

        #[doc(hidden)]
        #[allow(dead_code, non_snake_case)]
        #vis fn #helper_ident<S>(router: #arvik::Router<S>) -> #arvik::Router<S>
        where
            S: Clone + Send + Sync + 'static,
        {
            router.route_collected(#normalized_path, #arvik::#constructor(#fn_ident))
        }

        #[doc(hidden)]
        #[allow(dead_code, non_upper_case_globals)]
        #vis const #meta_ident: #arvik::__private::RouteMeta =
            #arvik::__private::RouteMeta::new(
                #normalized_path,
                #arvik::MethodFilter::#method_filter.bits(),
            );
    }
    .into()
}

fn expand_collect_routes(routes: RouteList) -> TokenStream2 {
    let arvik = arvik_path();
    let mut helpers = Vec::new();
    let mut metas = Vec::new();
    let mut errors = Vec::new();

    for path in routes.paths {
        match route_paths(path) {
            Ok((helper, meta)) => {
                helpers.push(helper);
                metas.push(meta);
            }
            Err(error) => errors.push(error.to_compile_error()),
        }
    }

    if !errors.is_empty() {
        return quote! {
            #(#errors)*
        };
    }

    quote! {
        {
            const _: () = {
                #arvik::__private::assert_no_route_conflicts(&[
                    #(#metas),*
                ]);
            };

            |router| {
                #(let router = #helpers(router);)*
                router
            }
        }
    }
}

fn expand_handler(input: ItemImpl) -> TokenStream2 {
    let arvik = arvik_path();
    let mut errors = Vec::new();

    if input.trait_.is_some() {
        errors.push(Error::new_spanned(
            input.impl_token,
            "#[handler] must be used on an inherent impl block",
        ));
    }

    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        errors.push(Error::new_spanned(
            &input.generics,
            "#[handler] does not support generic impl blocks yet",
        ));
    }

    let call_methods: Vec<&syn::ImplItemFn> = input
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(function) if function.sig.ident == "call" => Some(function),
            _ => None,
        })
        .collect();

    match call_methods.as_slice() {
        [] => errors.push(Error::new_spanned(
            &input.self_ty,
            "#[handler] requires an async fn call(&self, req: arvik::Request)",
        )),
        [call] => validate_handler_call(call, &mut errors),
        _ => errors.push(Error::new_spanned(
            &input.self_ty,
            "#[handler] requires exactly one method named call",
        )),
    }

    if !errors.is_empty() {
        let error_tokens = errors.into_iter().map(|error| error.to_compile_error());
        return quote! {
            #input
            #(#error_tokens)*
        };
    }

    let self_ty = &input.self_ty;

    quote! {
        #input

        impl<S> #arvik::Handler<(#arvik::Request,), S> for #self_ty
        where
            #self_ty: Clone + Send + Sync + 'static,
            S: Clone + Send + Sync + 'static,
        {
            type Future = ::std::pin::Pin<
                Box<dyn ::std::future::Future<Output = #arvik::Response> + Send + 'static>
            >;

            fn call(self, req: #arvik::Request, _state: S) -> Self::Future {
                Box::pin(async move {
                    #arvik::IntoResponse::into_response(<#self_ty>::call(&self, req).await)
                })
            }
        }
    }
}

fn validate_handler_call(call: &syn::ImplItemFn, errors: &mut Vec<Error>) {
    if call.sig.asyncness.is_none() {
        errors.push(Error::new_spanned(
            call.sig.fn_token,
            "#[handler] call method must be async",
        ));
    }

    if !call.sig.generics.params.is_empty() || call.sig.generics.where_clause.is_some() {
        errors.push(Error::new_spanned(
            &call.sig.generics,
            "#[handler] call method must not be generic",
        ));
    }

    if call.sig.inputs.len() != 2 {
        errors.push(Error::new_spanned(
            &call.sig.inputs,
            "#[handler] call method must have signature call(&self, req: arvik::Request)",
        ));
        return;
    }

    let mut inputs = call.sig.inputs.iter();
    match inputs.next() {
        Some(FnArg::Receiver(receiver))
            if receiver.reference.is_some() && receiver.mutability.is_none() => {}
        Some(arg) => errors.push(Error::new_spanned(
            arg,
            "#[handler] call method must take &self as the first argument",
        )),
        None => {}
    }

    match inputs.next() {
        Some(FnArg::Typed(arg)) if is_request_type(&arg.ty) => {}
        Some(arg) => errors.push(Error::new_spanned(
            arg,
            "#[handler] call method second argument must be arvik::Request",
        )),
        None => {}
    }
}

#[derive(Debug, Clone, Copy)]
enum RouteMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    Trace,
    Any,
}

impl RouteMethod {
    fn parse(method: &str, span: Span) -> std::result::Result<Self, TokenStream2> {
        match method.to_ascii_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "DELETE" => Ok(Self::Delete),
            "PATCH" => Ok(Self::Patch),
            "HEAD" => Ok(Self::Head),
            "OPTIONS" => Ok(Self::Options),
            "TRACE" => Ok(Self::Trace),
            "ANY" => Ok(Self::Any),
            _ => Err(Error::new(span, "unsupported HTTP method").to_compile_error()),
        }
    }

    fn constructor_ident(self) -> Ident {
        match self {
            Self::Get => format_ident!("get"),
            Self::Post => format_ident!("post"),
            Self::Put => format_ident!("put"),
            Self::Delete => format_ident!("delete"),
            Self::Patch => format_ident!("patch"),
            Self::Head => format_ident!("head"),
            Self::Options => format_ident!("options"),
            Self::Trace => format_ident!("trace_method"),
            Self::Any => format_ident!("any"),
        }
    }

    fn method_filter_ident(self) -> Ident {
        match self {
            Self::Get => format_ident!("GET"),
            Self::Post => format_ident!("POST"),
            Self::Put => format_ident!("PUT"),
            Self::Delete => format_ident!("DELETE"),
            Self::Patch => format_ident!("PATCH"),
            Self::Head => format_ident!("HEAD"),
            Self::Options => format_ident!("OPTIONS"),
            Self::Trace => format_ident!("TRACE"),
            Self::Any => format_ident!("ANY"),
        }
    }
}

fn route_paths(path: Path) -> Result<(Path, Path)> {
    let helper = replace_last_path_ident(&path, route_helper_ident)?;
    let meta = replace_last_path_ident(&path, route_meta_ident)?;
    Ok((helper, meta))
}

fn replace_last_path_ident(path: &Path, make_ident: fn(&Ident) -> Ident) -> Result<Path> {
    let mut path = path.clone();
    let path_span = path.span();
    let last = path
        .segments
        .last_mut()
        .ok_or_else(|| Error::new(path_span, "expected route handler path"))?;

    if !matches!(last.arguments, PathArguments::None) {
        return Err(Error::new_spanned(
            &last.arguments,
            "collect_routes! does not support generic route paths",
        ));
    }

    last.ident = make_ident(&last.ident);
    Ok(path)
}

fn route_helper_ident(ident: &Ident) -> Ident {
    format_ident!("__arvik_route_{}", ident)
}

fn route_meta_ident(ident: &Ident) -> Ident {
    let name = ident.to_string().to_ascii_uppercase();
    format_ident!("__ARVIK_ROUTE_META_{}", name)
}

fn normalize_route_path(path: &str) -> std::result::Result<String, String> {
    if path.is_empty() {
        return Err("route path must not be empty".to_string());
    }
    if !path.starts_with('/') {
        return Err("route path must start with `/`".to_string());
    }
    if path.contains('?') || path.contains('#') {
        return Err("route path must not contain a query string or fragment".to_string());
    }

    let mut normalized = String::with_capacity(path.len());
    for (idx, segment) in path.split('/').enumerate() {
        if idx > 0 {
            normalized.push('/');
        }

        if let Some(name) = segment.strip_prefix(':') {
            if name.is_empty()
                || !name
                    .chars()
                    .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            {
                return Err("route `:param` segments must be valid identifiers".to_string());
            }
            normalized.push('{');
            normalized.push_str(name);
            normalized.push('}');
        } else {
            normalized.push_str(segment);
        }
    }

    Ok(normalized)
}

fn arvik_path() -> TokenStream2 {
    match proc_macro_crate::crate_name("arvik") {
        Ok(proc_macro_crate::FoundCrate::Itself) => quote!(::arvik),
        Ok(proc_macro_crate::FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Err(_) => quote!(::arvik),
    }
}

fn is_state_type(ty: &Type) -> bool {
    last_type_segment_ident(ty).is_some_and(|ident| ident == "State")
}

fn is_request_type(ty: &Type) -> bool {
    last_type_segment_ident(ty).is_some_and(|ident| ident == "Request")
}

fn is_builtin_body_type(ty: &Type) -> bool {
    last_type_segment_ident(ty).is_some_and(|ident| {
        matches!(
            ident.to_string().as_str(),
            "Json" | "Form" | "Multipart" | "Body" | "Bytes" | "String" | "Request"
        )
    })
}

fn last_type_segment_ident(ty: &Type) -> Option<&Ident> {
    match ty {
        Type::Path(path) => path.path.segments.last().map(|segment| &segment.ident),
        Type::Reference(reference) => last_type_segment_ident(&reference.elem),
        _ => None,
    }
}
