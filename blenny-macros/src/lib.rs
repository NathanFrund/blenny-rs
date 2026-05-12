use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, ExprArray, Ident, ItemStruct, Lit, LitBool, LitStr, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
};

/// Parsed attributes: #[blenny_module(route_handler = "...", path = "...", scope = "...", enable = ..., public_routes = [...], initialize_handler = "...")]
struct BlennyModuleAttrs {
    path: Option<LitStr>,
    scope: Option<LitStr>,
    route_handler: Option<LitStr>,
    enable: Option<LitBool>,
    public_routes: Option<ExprArray>,
    initialize_handler: Option<LitStr>,
}

impl Parse for BlennyModuleAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut path = None;
        let mut scope = None;
        let mut route_handler = None;
        let mut enable = None;
        let mut public_routes = None;
        let mut initialize_handler = None;

        let metas: Punctuated<syn::Meta, Token![,]> =
            input.parse_terminated(syn::Meta::parse, Token![,])?;
        for meta in metas {
            if meta.path().is_ident("path") {
                if let syn::Meta::NameValue(nv) = meta {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Str(s) = &expr_lit.lit {
                            path = Some(s.clone());
                        } else {
                            return Err(syn::Error::new(
                                nv.value.span(),
                                "expected string literal for path",
                            ));
                        }
                    } else {
                        return Err(syn::Error::new(
                            nv.value.span(),
                            "expected string literal for path",
                        ));
                    }
                } else {
                    return Err(syn::Error::new(meta.span(), "expected path = \"...\""));
                }
            } else if meta.path().is_ident("scope") {
                if let syn::Meta::NameValue(nv) = meta {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Str(s) = &expr_lit.lit {
                            scope = Some(s.clone());
                        } else {
                            return Err(syn::Error::new(
                                nv.value.span(),
                                "expected string literal for scope",
                            ));
                        }
                    } else {
                        return Err(syn::Error::new(
                            nv.value.span(),
                            "expected string literal for scope",
                        ));
                    }
                } else {
                    return Err(syn::Error::new(meta.span(), "expected scope = \"...\""));
                }
            } else if meta.path().is_ident("route_handler") {
                if let syn::Meta::NameValue(nv) = meta {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Str(s) = &expr_lit.lit {
                            route_handler = Some(s.clone());
                        } else {
                            return Err(syn::Error::new(
                                nv.value.span(),
                                "expected string literal for route_handler",
                            ));
                        }
                    } else {
                        return Err(syn::Error::new(
                            nv.value.span(),
                            "expected string literal for route_handler",
                        ));
                    }
                } else {
                    return Err(syn::Error::new(
                        meta.span(),
                        "expected route_handler = \"fn_name\"",
                    ));
                }
            } else if meta.path().is_ident("enable") {
                if let syn::Meta::NameValue(nv) = meta {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Bool(b) = &expr_lit.lit {
                            enable = Some(b.clone());
                        } else {
                            return Err(syn::Error::new(
                                nv.value.span(),
                                "expected boolean literal for enable",
                            ));
                        }
                    } else {
                        return Err(syn::Error::new(
                            nv.value.span(),
                            "expected boolean literal for enable",
                        ));
                    }
                } else {
                    return Err(syn::Error::new(meta.span(), "expected enable = true/false"));
                }
            } else if meta.path().is_ident("public_routes") {
                if let syn::Meta::NameValue(nv) = meta {
                    if let Expr::Array(arr) = &nv.value {
                        public_routes = Some(ExprArray {
                            attrs: vec![],
                            bracket_token: syn::token::Bracket::default(),
                            elems: arr.elems.clone(),
                        });
                    } else {
                        return Err(syn::Error::new(
                            nv.value.span(),
                            "expected array literal for public_routes",
                        ));
                    }
                } else {
                    return Err(syn::Error::new(
                        meta.span(),
                        "expected public_routes = [...]",
                    ));
                }
            } else if meta.path().is_ident("initialize_handler") {
                if let syn::Meta::NameValue(nv) = meta {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Str(s) = &expr_lit.lit {
                            initialize_handler = Some(s.clone());
                        } else {
                            return Err(syn::Error::new(
                                nv.value.span(),
                                "expected string literal for initialize_handler",
                            ));
                        }
                    } else {
                        return Err(syn::Error::new(
                            nv.value.span(),
                            "expected string literal for initialize_handler",
                        ));
                    }
                } else {
                    return Err(syn::Error::new(
                        meta.span(),
                        "expected initialize_handler = \"fn_name\"",
                    ));
                }
            }
            // ignore unknown meta for forward compat
        }

        Ok(BlennyModuleAttrs {
            path,
            scope,
            route_handler,
            enable,
            public_routes,
            initialize_handler,
        })
    }
}

#[proc_macro_attribute]
pub fn blenny_module(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let name = &input.ident;

    let attrs = match syn::parse::<BlennyModuleAttrs>(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    let prefix = attrs.path.or(attrs.scope).map(|s| s.value());
    let prefix_tokens = prefix
        .map(|p| quote! { Some(#p) })
        .unwrap_or_else(|| quote! { None });

    let public_routes: Vec<String> = attrs
        .public_routes
        .map(|arr| {
            arr.elems
                .into_iter()
                .filter_map(|e| {
                    if let syn::Expr::Lit(el) = e {
                        if let syn::Lit::Str(s) = el.lit {
                            return Some(s.value());
                        }
                    }
                    None
                })
                .collect()
        })
        .unwrap_or_default();

    let public_routes_tokens = if public_routes.is_empty() {
        quote! { &[] as &[&str] }
    } else {
        let strings: Vec<proc_macro2::TokenStream> = public_routes
            .iter()
            .map(|s| {
                let s = s.as_str();
                quote! { #s }
            })
            .collect();
        quote! { &[#(#strings),*] as &[&str] }
    };

    let enabled_tokens = attrs.enable.map_or(quote! { true }, |b| {
        let val = b.value;
        quote! { #val }
    });

    let route_handler_tokens = attrs.route_handler.map_or_else(
        || {
            quote! {
                fn register_routes(&self, router: ::axum::Router) -> ::axum::Router {
                    router
                }
            }
        },
        |func_str| {
            let func_ident = Ident::new(&func_str.value(), func_str.span());
            quote! {
                fn register_routes(&self, router: ::axum::Router) -> ::axum::Router {
                    Self::#func_ident(router)
                }
            }
        },
    );

    // Optional: use LazyLock for public routes performance
    let public_routes_method = if public_routes.is_empty() {
        quote! {
            fn public_routes(&self) -> std::collections::HashSet<String> {
                std::collections::HashSet::new()
            }
        }
    } else {
        // Generate a static HashSet using LazyLock (Rust 1.80+)
        let inserts = public_routes.iter().map(|s| {
            quote! { set.insert(#s.to_string()); }
        });
        quote! {
            fn public_routes(&self) -> std::collections::HashSet<String> {
                use std::sync::LazyLock;
                static ROUTES: std::sync::LazyLock<std::collections::HashSet<String>> = std::sync::LazyLock::new(|| {
                    let mut set = std::collections::HashSet::new();
                    #(#inserts)*
                    set
                });
                ROUTES.clone()
            }
        }
    };

    let initialize_method = attrs.initialize_handler.map_or_else(|| quote! {}, |init_str| {
        let init_ident = Ident::new(&init_str.value(), init_str.span());
        quote! {
            fn initialize_module(&mut self, state: ::std::sync::Arc<::blenny::app_state::AppState>) {
                self.#init_ident(state);
            }
        }
    });

    let expanded = quote! {
        #input

        impl ::blenny::BlennyModule for #name {
            fn name(&self) -> &'static str {
                stringify!(#name)
            }
            fn is_enabled(&self) -> bool {
                #enabled_tokens
            }
            #public_routes_method
            #route_handler_tokens
            #initialize_method
        }

        inventory::submit! {
            blenny::module::ModuleRegistration {
                name: stringify!(#name),
                constructor: || Box::new(#name::default()),
                prefix: #prefix_tokens,
                public_routes: #public_routes_tokens,
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn blenny_auth_provider(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let name = &input.ident;

    let expanded = quote! {
        #input

        inventory::submit! {
            blenny::auth::AuthRegistration {
                name: stringify!(#name),
                constructor: || {
                    std::sync::Arc::new(#name::default())
                        as std::sync::Arc<dyn blenny::auth::AuthProvider>
                },
            }
        }
    };

    TokenStream::from(expanded)
}
