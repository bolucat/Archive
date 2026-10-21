use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use serde_derive_internals::{Ctxt, Derive, ast, attr};
use syn::{DeriveInput, Meta, Token, parse_macro_input, punctuated::Punctuated};

mod untagged;

/// Derive direct decoding for scalar untagged enums and open string enums.
#[proc_macro_derive(DeserializeUntagged, attributes(serde, nyanpasu_serde))]
pub fn deserialize_untagged(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    runtime_path(&input)
        .and_then(|runtime| untagged::generate(&input, runtime))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive map decoding with an optional flattened field group and an explicit extra sink.
#[proc_macro_derive(DeserializeMap, attributes(serde, nyanpasu_serde))]
pub fn deserialize_map(input: TokenStream) -> TokenStream {
    expand(&parse_macro_input!(input as DeriveInput), true)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive a field group that can be flattened by `DeserializeMap` without buffering.
#[proc_macro_derive(DeserializeFields, attributes(serde, nyanpasu_serde))]
pub fn deserialize_fields(input: TokenStream) -> TokenStream {
    expand(&parse_macro_input!(input as DeriveInput), false)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn validate_attributes(attributes: &[syn::Attribute], allowed: &[&str]) -> syn::Result<()> {
    for attribute in attributes
        .iter()
        .filter(|attr| attr.path().is_ident("serde"))
    {
        let items = attribute.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for item in items {
            if !allowed.iter().any(|name| item.path().is_ident(name)) {
                return Err(syn::Error::new_spanned(
                    item,
                    "unsupported Serde attribute for direct map decoding",
                ));
            }
        }
    }
    Ok(())
}

fn extra(field: &syn::Field) -> syn::Result<bool> {
    let mut extra = false;
    for attribute in field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("nyanpasu_serde"))
    {
        attribute.parse_nested_meta(|meta| {
            if !meta.path.is_ident("extra") || extra {
                return Err(meta.error("expected a single `extra` marker"));
            }
            extra = true;
            Ok(())
        })?;
    }
    Ok(extra)
}

fn reject_recursion(ty: &syn::Type, name: &syn::Ident) -> syn::Result<()> {
    use syn::visit::Visit;

    struct Recursion<'a>(&'a syn::Ident, bool);
    impl<'ast> Visit<'ast> for Recursion<'_> {
        fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
            if path
                .path
                .segments
                .iter()
                .any(|segment| segment.ident == "Self" || segment.ident == *self.0)
            {
                self.1 = true;
            }
            syn::visit::visit_type_path(self, path);
        }
    }
    let mut recursion = Recursion(name, false);
    recursion.visit_type(ty);
    if recursion.1 {
        return Err(syn::Error::new_spanned(
            ty,
            "recursive fields are unsupported by direct map derives",
        ));
    }
    Ok(())
}

fn expand(input: &DeriveInput, deserialize: bool) -> syn::Result<Tokens> {
    generate(input, deserialize, runtime_path(input)?)
}

fn runtime_path(input: &DeriveInput) -> syn::Result<Tokens> {
    let runtime = match proc_macro_crate::crate_name("nyanpasu-serde")
        .map_err(|error| syn::Error::new_spanned(input, error))?
    {
        proc_macro_crate::FoundCrate::Itself => quote!(::nyanpasu_serde),
        proc_macro_crate::FoundCrate::Name(name) => {
            let name = format_ident!("{}", name);
            quote!(::#name)
        }
    };
    Ok(runtime)
}

fn generate(input: &DeriveInput, deserialize: bool, runtime: Tokens) -> syn::Result<Tokens> {
    validate_attributes(&input.attrs, &["rename", "rename_all"])?;
    if let Some(attribute) = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("nyanpasu_serde"))
    {
        return Err(syn::Error::new_spanned(
            attribute,
            "extra is a field attribute",
        ));
    }
    if let syn::Data::Struct(data) = &input.data {
        for field in &data.fields {
            validate_attributes(
                &field.attrs,
                &[
                    "rename",
                    "alias",
                    "default",
                    "flatten",
                    "with",
                    "deserialize_with",
                    "serialize_with",
                    "skip_serializing",
                    "skip_serializing_if",
                    "skip_deserializing",
                ],
            )?;
        }
    }
    let context = Ctxt::new();
    let container = ast::Container::from_ast(
        &context,
        input,
        Derive::Deserialize,
        &format_ident!("_serde"),
    );
    context.check()?;
    let container = container.ok_or_else(|| syn::Error::new_spanned(input, "expected a struct"))?;
    let ast::Data::Struct(ast::Style::Struct, fields) = container.data else {
        return Err(syn::Error::new_spanned(
            input,
            "direct map derives require named structs",
        ));
    };

    let private = quote!(#runtime::__private);
    let serde = quote!(#private::serde);
    let name = &input.ident;
    let (_, original_types, _) = input.generics.split_for_impl();
    let original = quote!(#name #original_types);
    let mut generics = input.generics.clone();
    generics.params.insert(0, syn::parse_quote!('__nyanpasu_de));
    let (_, seed_types, _) = generics.split_for_impl();
    let seed_types = quote!(#seed_types);
    let mut state_fields = Vec::new();
    let mut defaults = Vec::new();
    let mut lookup = Vec::new();
    let mut reads = Vec::new();
    let mut finishes = Vec::new();
    let mut seeds = Vec::new();
    let mut flattened = None;
    let mut extras = None;
    let mut index = 0usize;
    let mut names = std::collections::BTreeSet::new();
    let mut marker = format_ident!("__marker");
    while fields
        .iter()
        .any(|field| field.member == syn::Member::Named(marker.clone()))
    {
        marker = format_ident!("_{}", marker);
    }

    for field in &fields {
        reject_recursion(field.ty, name)?;
        let member = &field.member;
        let ty = field.ty;
        let is_extra = extra(field.original)?;
        if is_extra || field.attrs.flatten() {
            if !matches!(field.attrs.default(), attr::Default::None)
                || field.attrs.deserialize_with().is_some()
                || field.attrs.skip_deserializing()
            {
                return Err(syn::Error::new_spanned(
                    field.original,
                    "flattened fields cannot have decoding modifiers",
                ));
            }
            if is_extra {
                if !deserialize || extras.is_some() || !field.attrs.flatten() {
                    return Err(syn::Error::new_spanned(
                        field.original,
                        "one #[serde(flatten)] extra sink is allowed on DeserializeMap",
                    ));
                }
                generics
                    .make_where_clause()
                    .predicates
                    .push(syn::parse_quote!(#ty: #private::ExtraFields<'__nyanpasu_de>));
                state_fields.push(quote!(#member: #ty));
                defaults.push(quote!(#member: ::core::default::Default::default()));
                finishes.push(quote!(#member: state.#member));
                extras = Some(member);
            } else {
                if flattened.is_some() {
                    return Err(syn::Error::new_spanned(
                        field.original,
                        "only one flattened field group is supported",
                    ));
                }
                generics
                    .make_where_clause()
                    .predicates
                    .push(syn::parse_quote!(#ty: #private::KnownFields<'__nyanpasu_de>));
                state_fields
                    .push(quote!(#member: <#ty as #private::MapFields<'__nyanpasu_de>>::State));
                defaults.push(quote!(#member: ::core::default::Default::default()));
                finishes.push(quote!(#member: <#ty as #private::MapFields<'__nyanpasu_de>>::finish(state.#member)?));
                flattened = Some((member, ty));
            }
            continue;
        }
        if field.attrs.skip_deserializing() {
            let default = match field.attrs.default() {
                attr::Default::Path(path) => quote!(#path()),
                _ => {
                    generics
                        .make_where_clause()
                        .predicates
                        .push(syn::parse_quote!(#ty: ::core::default::Default));
                    quote!(::core::default::Default::default())
                }
            };
            finishes.push(quote!(#member: #default));
            continue;
        }
        state_fields.push(quote!(#member: ::core::option::Option<#ty>));
        defaults.push(quote!(#member: ::core::option::Option::None));
        let field_name = field.attrs.name().deserialize_name();
        let aliases = field.attrs.aliases();
        for alias in aliases {
            if !names.insert(alias) {
                return Err(syn::Error::new_spanned(
                    field.original,
                    "overlapping field names or aliases",
                ));
            }
        }
        lookup.push(quote!(#(#aliases)|* => ::core::option::Option::Some(#index),));
        let value = if let Some(path) = field.attrs.deserialize_with() {
            let seed = format_ident!("__Seed{}", index);
            seeds.push((seed.clone(), ty, path));
            quote!(map.next_value_seed(#seed :: #seed_types (::core::marker::PhantomData))?)
        } else {
            generics
                .make_where_clause()
                .predicates
                .push(syn::parse_quote!(#ty: #serde::Deserialize<'__nyanpasu_de>));
            quote!(map.next_value()?)
        };
        reads.push(quote! {
            #index => {
                if state.#member.is_some() {
                    return ::core::result::Result::Err(#serde::de::Error::duplicate_field(#field_name));
                }
                state.#member = ::core::option::Option::Some(#value);
                ::core::result::Result::Ok(())
            }
        });
        let missing = match field.attrs.default() {
            attr::Default::Default => {
                generics
                    .make_where_clause()
                    .predicates
                    .push(syn::parse_quote!(#ty: ::core::default::Default));
                quote!(::core::default::Default::default())
            }
            attr::Default::Path(path) => quote!(#path()),
            attr::Default::None if field.attrs.deserialize_with().is_some() => {
                quote!(return ::core::result::Result::Err(#serde::de::Error::missing_field(#field_name)))
            }
            attr::Default::None => quote!(#private::missing(#field_name)?),
        };
        finishes.push(quote!(#member: match state.#member {
            ::core::option::Option::Some(value) => value,
            ::core::option::Option::None => #missing,
        }));
        index += 1;
    }
    let (implementation, types, constraints) = generics.split_for_impl();
    let mut seed_impls = Vec::new();
    for (seed, ty, path) in seeds {
        seed_impls.push(quote! {
            struct #seed #generics (::core::marker::PhantomData<fn() -> (&'__nyanpasu_de (), #original)>) #constraints;
            impl #implementation #serde::de::DeserializeSeed<'__nyanpasu_de> for #seed #types #constraints {
                type Value = #ty;
                #[inline(always)]
                fn deserialize<__D: #serde::Deserializer<'__nyanpasu_de>>(self, deserializer: __D) -> ::core::result::Result<Self::Value, __D::Error> {
                    #path(deserializer)
                }
            }
        });
    }
    let fallback_lookup = flattened.map(|(_, ty)| quote!(<#ty as #private::MapFields<'__nyanpasu_de>>::field(name).map(|field| field + #index)))
        .unwrap_or_else(|| quote!(::core::option::Option::None));
    let fallback_read = flattened.map(|(member, ty)| quote!(<#ty as #private::MapFields<'__nyanpasu_de>>::read(&mut state.#member, field - #index, map)))
        .unwrap_or_else(|| quote!(unreachable!("invalid generated field index")));
    let unknown = extras.map(|member| quote!(#private::ExtraFields::read(&mut state.#member, key, map)))
        .unwrap_or_else(|| quote!({ let _ = (state, key); map.next_value::<#serde::de::IgnoredAny>()?; ::core::result::Result::Ok(()) }));
    let deserialize_impl = deserialize.then(|| quote! {
        impl #implementation #serde::Deserialize<'__nyanpasu_de> for #original #constraints {
            #[inline(always)]
            fn deserialize<__D: #serde::Deserializer<'__nyanpasu_de>>(deserializer: __D) -> ::core::result::Result<Self, __D::Error> {
                #private::deserialize_map(deserializer)
            }
        }
    });
    let known_impl = (!deserialize).then(|| {
        quote! {
            impl #implementation #private::KnownFields<'__nyanpasu_de> for #original #constraints {}
        }
    });
    Ok(quote! {
        const _: () = {
            pub struct __State #generics #constraints {
                #(#state_fields,)*
                #marker: ::core::marker::PhantomData<fn() -> (&'__nyanpasu_de (), #original)>,
            }
            impl #implementation ::core::default::Default for __State #types #constraints {
                #[inline(always)]
                fn default() -> Self {
                    Self { #(#defaults,)* #marker: ::core::marker::PhantomData }
                }
            }
            #(#seed_impls)*
            impl #implementation #private::MapFields<'__nyanpasu_de> for #original #constraints {
                type State = __State #types;
                #[inline(always)]
                fn field(name: &str) -> ::core::option::Option<usize> {
                    match name { #(#lookup)* _ => #fallback_lookup }
                }
                #[inline(always)]
                fn read<__M: #serde::de::MapAccess<'__nyanpasu_de>>(state: &mut Self::State, field: usize, map: &mut __M) -> ::core::result::Result<(), __M::Error> {
                    match field { #(#reads)* _ => { #fallback_read } }
                }
                #[inline(always)]
                fn unknown<__M: #serde::de::MapAccess<'__nyanpasu_de>>(state: &mut Self::State, key: ::std::string::String, map: &mut __M) -> ::core::result::Result<(), __M::Error> {
                    #unknown
                }
                #[inline(always)]
                fn finish<__E: #serde::de::Error>(state: Self::State) -> ::core::result::Result<Self, __E> {
                    ::core::result::Result::Ok(Self { #(#finishes,)* })
                }
            }
            #deserialize_impl
            #known_impl
        };
    })
}

#[cfg(test)]
mod tests {
    use syn::visit::Visit;

    use super::*;

    #[test]
    fn unsupported_shapes_and_attributes_fail_closed() {
        for (input, message) in [
            (
                quote!(
                    struct Model {
                        next: Option<Box<Self>>,
                    }
                ),
                "recursive fields",
            ),
            (
                quote!(
                    struct Model {
                        #[serde(alias = "b")]
                        a: u8,
                        b: u8,
                    }
                ),
                "overlapping field names",
            ),
            (
                quote!(
                    struct Model(u32);
                ),
                "named structs",
            ),
            (
                quote!(
                    enum Model {
                        A,
                    }
                ),
                "named structs",
            ),
            (
                quote!(
                    #[serde(default)]
                    struct Model {
                        a: u32,
                    }
                ),
                "unsupported Serde attribute",
            ),
            (
                quote!(
                    #[serde(from = "Other")]
                    struct Model {
                        a: u32,
                    }
                ),
                "unsupported Serde attribute",
            ),
            (
                quote!(
                    struct Model {
                        #[serde(borrow)]
                        a: String,
                    }
                ),
                "unsupported Serde attribute",
            ),
            (
                quote!(
                    struct Model {
                        #[serde(flatten)]
                        a: A,
                        #[serde(flatten)]
                        b: B,
                    }
                ),
                "only one flattened",
            ),
            (
                quote!(
                    struct Model {
                        #[nyanpasu_serde(extra)]
                        a: A,
                    }
                ),
                "extra sink",
            ),
            (
                quote!(
                    struct Model {
                        #[serde(flatten, default)]
                        a: A,
                    }
                ),
                "decoding modifiers",
            ),
        ] {
            let input = syn::parse2(input).unwrap();
            let error = generate(&input, true, quote!(::nyanpasu_serde)).unwrap_err();
            assert!(error.to_string().contains(message), "{error}");
        }
        let input = syn::parse_quote!(
            struct Model {
                #[serde(flatten)]
                #[nyanpasu_serde(extra)]
                extra: A,
            }
        );
        assert!(generate(&input, false, quote!(::nyanpasu_serde)).is_err());
    }

    #[test]
    fn every_generated_method_is_always_inline() {
        struct Methods(usize);
        impl<'ast> Visit<'ast> for Methods {
            fn visit_impl_item_fn(&mut self, method: &'ast syn::ImplItemFn) {
                assert!(
                    method.attrs.iter().any(|attribute| {
                        attribute.path().is_ident("inline")
                            && attribute
                                .parse_args::<syn::Ident>()
                                .is_ok_and(|arg| arg == "always")
                    }),
                    "{}",
                    method.sig.ident
                );
                self.0 += 1;
            }
        }
        let input = syn::parse_quote!(
            struct Model<T> {
                value: T,
                #[serde(deserialize_with = "decode")]
                custom: Custom,
            }
        );
        let expanded = generate(&input, true, quote!(::nyanpasu_serde)).unwrap();
        let file: syn::File = syn::parse2(expanded).unwrap();
        let mut methods = Methods(0);
        methods.visit_file(&file);
        assert_eq!(methods.0, 7);
        for input in [
            syn::parse_quote!(
                #[serde(untagged)]
                enum Scalar {
                    Number(u16),
                    #[nyanpasu_serde(kind = "string")]
                    Text(#[serde(deserialize_with = "parse")] u16),
                    Null,
                }
            ),
            syn::parse_quote!(
                enum Open {
                    Known,
                    #[serde(untagged)]
                    Other(String),
                }
            ),
        ] {
            let expanded = untagged::generate(&input, quote!(::nyanpasu_serde)).unwrap();
            let file: syn::File = syn::parse2(expanded).unwrap();
            let mut methods = Methods(0);
            methods.visit_file(&file);
            assert!(methods.0 >= 3);
        }
    }

    #[test]
    fn untagged_derive_rejects_ambiguous_shapes_and_unsupported_attributes() {
        for input in [
            quote!(
                struct Model {
                    value: u8,
                }
            ),
            quote!(
                #[serde(untagged)]
                enum Model {
                    A { x: u8 },
                    B { y: u8 },
                }
            ),
            quote!(
                #[serde(untagged)]
                enum Model {
                    Pair(u8, u8),
                }
            ),
            quote!(
                #[serde(untagged)]
                enum Model<T> {
                    Unknown(T),
                }
            ),
            quote!(
                #[serde(untagged)]
                enum Model {
                    Unknown(custom::String),
                }
            ),
            quote!(
                #[serde(untagged)]
                enum Model {
                    #[nyanpasu_serde(kind = "map")]
                    A(u8),
                }
            ),
            quote!(
                #[serde(untagged)]
                enum Model {
                    #[nyanpasu_serde(kind = "string")]
                    A(u8),
                }
            ),
            quote!(
                #[serde(untagged)]
                enum Model {
                    A(#[serde(deserialize_with = "parse")] u8),
                }
            ),
            quote!(
                #[serde(untagged)]
                enum Model {
                    A(#[nyanpasu_serde(kind = "number")] u8),
                }
            ),
            quote!(
                #[serde(tag = "type")]
                enum Model {
                    A,
                }
            ),
            quote!(
                enum Model {
                    A,
                    #[serde(untagged)]
                    Unknown(Vec<u8>),
                }
            ),
            quote!(
                enum Model {
                    #[serde(alias = "B")]
                    A,
                    B,
                    #[serde(untagged)]
                    Unknown(String),
                }
            ),
            quote!(
                enum Model {
                    A,
                    #[serde(untagged)]
                    Unknown(#[serde(deserialize_with = "parse")] String),
                }
            ),
        ] {
            let input = syn::parse2(input).unwrap();
            assert!(untagged::generate(&input, quote!(::nyanpasu_serde)).is_err());
        }
    }
}
