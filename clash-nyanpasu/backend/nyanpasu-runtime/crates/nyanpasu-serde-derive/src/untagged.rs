use proc_macro2::TokenStream;
use quote::quote;
use serde_derive_internals::{Ctxt, Derive, ast, attr};
use syn::{DeriveInput, Type};

use super::validate_attributes;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    String,
    Number,
    Bool,
    Null,
}

fn inferred_kind(ty: &Type) -> Option<Kind> {
    let Type::Path(path) = ty else { return None };
    if path.qself.is_some() {
        return None;
    }
    let name = path.path.segments.last()?.ident.to_string();
    let segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    if path
        .path
        .segments
        .iter()
        .any(|segment| !matches!(segment.arguments, syn::PathArguments::None))
    {
        return None;
    }
    let prefix = &segments[..segments.len() - 1];
    let standard = prefix.is_empty()
        || (name == "String" && (prefix == ["std", "string"] || prefix == ["alloc", "string"]))
        || (name != "String"
            && (prefix == ["std", "primitive"] || prefix == ["core", "primitive"]));
    if !standard {
        return None;
    }
    match name.as_str() {
        "String" => Some(Kind::String),
        "bool" => Some(Kind::Bool),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128"
        | "isize" | "f32" | "f64" => Some(Kind::Number),
        _ => None,
    }
}

fn kind(variant: &syn::Variant) -> syn::Result<Option<Kind>> {
    let mut kind = None;
    for attribute in variant
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("nyanpasu_serde"))
    {
        attribute.parse_nested_meta(|meta| {
            if !meta.path.is_ident("kind") || kind.is_some() {
                return Err(meta.error("expected one kind = \"string\", \"number\", or \"bool\""));
            }
            let name: syn::LitStr = meta.value()?.parse()?;
            kind = Some(match name.value().as_str() {
                "string" => Kind::String,
                "number" => Kind::Number,
                "bool" => Kind::Bool,
                _ => return Err(syn::Error::new_spanned(name, "unsupported scalar kind")),
            });
            Ok(())
        })?;
    }
    Ok(kind)
}

pub(super) fn generate(input: &DeriveInput, runtime: TokenStream) -> syn::Result<TokenStream> {
    validate_attributes(&input.attrs, &["untagged", "rename", "rename_all"])?;
    if input
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("nyanpasu_serde"))
    {
        return Err(syn::Error::new_spanned(
            input,
            "scalar kind belongs on a variant",
        ));
    }
    let context = Ctxt::new();
    let container = ast::Container::from_ast(
        &context,
        input,
        Derive::Deserialize,
        &syn::parse_quote!(_serde),
    );
    context.check()?;
    let container = container.ok_or_else(|| syn::Error::new_spanned(input, "expected an enum"))?;
    let ast::Data::Enum(variants) = &container.data else {
        return Err(syn::Error::new_spanned(
            input,
            "DeserializeUntagged requires an enum",
        ));
    };
    for variant in variants {
        for field in &variant.fields {
            if field
                .original
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("nyanpasu_serde"))
            {
                return Err(syn::Error::new_spanned(
                    field.original,
                    "scalar kind belongs on the variant",
                ));
            }
        }
    }
    let private = quote!(#runtime::__private);
    let serde = quote!(#private::serde);
    let name = &input.ident;
    let (_, original_types, _) = input.generics.split_for_impl();
    let original = quote!(#name #original_types);
    let mut generics = input.generics.clone();
    generics.params.insert(0, syn::parse_quote!('__nyanpasu_de));

    if matches!(container.attrs.tag(), attr::TagType::External) {
        let mut cases = Vec::new();
        let mut names = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut fallback = None;
        for variant in variants {
            validate_attributes(
                &variant.original.attrs,
                &[
                    "rename",
                    "alias",
                    "untagged",
                    "skip_serializing",
                    "skip_deserializing",
                ],
            )?;
            if kind(variant.original)?.is_some() {
                return Err(syn::Error::new_spanned(
                    variant.original,
                    "open string enums do not use scalar kind markers",
                ));
            }
            if variant.attrs.skip_deserializing() {
                continue;
            }
            let ident = &variant.ident;
            match variant.style {
                ast::Style::Unit if !variant.attrs.untagged() => {
                    let aliases = variant.attrs.aliases();
                    for alias in aliases {
                        if !seen.insert(alias) {
                            return Err(syn::Error::new_spanned(
                                variant.original,
                                "overlapping variant names or aliases",
                            ));
                        }
                    }
                    names.push(variant.attrs.name().deserialize_name());
                    cases
                        .push(quote!(#(#aliases)|* => ::core::option::Option::Some(Self::#ident),));
                }
                ast::Style::Newtype
                    if variant.attrs.untagged()
                        && inferred_kind(variant.fields[0].ty) == Some(Kind::String)
                        && fallback.is_none() =>
                {
                    validate_attributes(&variant.fields[0].original.attrs, &[])?;
                    fallback = Some(ident);
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        variant.original,
                        "expected unit variants followed by one #[serde(untagged)] String fallback",
                    ));
                }
            }
        }
        let fallback = fallback.ok_or_else(|| {
            syn::Error::new_spanned(
                input,
                "an open string enum requires an untagged String fallback",
            )
        })?;
        let (implementation, _, constraints) = generics.split_for_impl();
        let (known_impl, _, known_constraints) = input.generics.split_for_impl();
        return Ok(quote! {
            const _: () = {
                impl #known_impl #private::StringEnum for #original #known_constraints {
                    const VARIANTS: &'static [&'static str] = &[#(#names),*];
                    #[inline(always)]
                    fn known(value: &str) -> ::core::option::Option<Self> {
                        match value { #(#cases)* _ => ::core::option::Option::None }
                    }
                    #[inline(always)]
                    fn unknown(value: ::std::string::String) -> Self { Self::#fallback(value) }
                }
                impl #implementation #serde::Deserialize<'__nyanpasu_de> for #original #constraints {
                    #[inline(always)]
                    fn deserialize<__D: #serde::Deserializer<'__nyanpasu_de>>(deserializer: __D) -> ::core::result::Result<Self, __D::Error> {
                        #private::deserialize_string_enum(deserializer)
                    }
                }
            };
        });
    }
    if !matches!(container.attrs.tag(), attr::TagType::None) {
        return Err(syn::Error::new_spanned(
            input,
            "expected #[serde(untagged)]",
        ));
    }
    let mut decoded = Vec::new();
    for variant in variants {
        validate_attributes(
            &variant.original.attrs,
            &["skip_serializing", "skip_deserializing"],
        )?;
        if variant.attrs.skip_deserializing() {
            continue;
        }
        let marker = kind(variant.original)?;
        let ident = &variant.ident;
        match variant.style {
            ast::Style::Unit if marker.is_none() => decoded.push((Kind::Null, ident, None)),
            ast::Style::Newtype => {
                let field = &variant.fields[0];
                validate_attributes(
                    &field.original.attrs,
                    &["with", "deserialize_with", "serialize_with"],
                )?;
                let kind = marker.or_else(|| inferred_kind(field.ty)).ok_or_else(|| syn::Error::new_spanned(field.ty, "generic or custom fields need an explicit #[nyanpasu_serde(kind = \"...\")] on the variant"))?;
                if field.attrs.deserialize_with().is_none() {
                    if marker.is_some()
                        && inferred_kind(field.ty).is_some_and(|inferred| inferred != kind)
                    {
                        return Err(syn::Error::new_spanned(
                            variant.original,
                            "scalar kind conflicts with the field type",
                        ));
                    }
                    let ty = field.ty;
                    generics
                        .make_where_clause()
                        .predicates
                        .push(syn::parse_quote!(#ty: #serde::Deserialize<'__nyanpasu_de>));
                } else if marker.is_none() {
                    return Err(syn::Error::new_spanned(
                        field.original,
                        "custom decoders need an explicit scalar kind",
                    ));
                }
                decoded.push((kind, ident, Some(field)));
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    variant.original,
                    "only scalar newtype and null unit variants are supported; object/sequence alternatives require buffering",
                ));
            }
        }
    }
    let (implementation, types, constraints) = generics.split_for_impl();
    let error = format!("data did not match any scalar variant of {}", name);
    let mut methods = Vec::new();
    // Candidate errors are discarded; avoid constructing format-specific errors
    // for failed alternatives. Only the final failure uses the caller's error type.
    for (method, ty, deserializer, kind) in [
        (
            "visit_i64",
            quote!(i64),
            quote!(#serde::de::value::I64Deserializer::<#serde::de::value::Error>::new(value)),
            Kind::Number,
        ),
        (
            "visit_u64",
            quote!(u64),
            quote!(#serde::de::value::U64Deserializer::<#serde::de::value::Error>::new(value)),
            Kind::Number,
        ),
        (
            "visit_i128",
            quote!(i128),
            quote!(#serde::de::value::I128Deserializer::<#serde::de::value::Error>::new(value)),
            Kind::Number,
        ),
        (
            "visit_u128",
            quote!(u128),
            quote!(#serde::de::value::U128Deserializer::<#serde::de::value::Error>::new(value)),
            Kind::Number,
        ),
        (
            "visit_f64",
            quote!(f64),
            quote!(#serde::de::value::F64Deserializer::<#serde::de::value::Error>::new(value)),
            Kind::Number,
        ),
        (
            "visit_bool",
            quote!(bool),
            quote!(#serde::de::value::BoolDeserializer::<#serde::de::value::Error>::new(value)),
            Kind::Bool,
        ),
        (
            "visit_str",
            quote!(&str),
            quote!(#serde::de::value::StrDeserializer::<#serde::de::value::Error>::new(value)),
            Kind::String,
        ),
        (
            "visit_borrowed_str",
            quote!(&'__nyanpasu_de str),
            quote!(#serde::de::value::BorrowedStrDeserializer::<#serde::de::value::Error>::new(value)),
            Kind::String,
        ),
    ] {
        let method = syn::Ident::new(method, name.span());
        let attempts = decoded.iter().filter(|(candidate, _, _)| *candidate == kind).map(|(_, ident, field)| {
            let field = field.unwrap();
            let ty = field.ty;
            let decode = field.attrs.deserialize_with().map(|path| quote!(#path(#deserializer)))
                .unwrap_or_else(|| quote!(<#ty as #serde::Deserialize<'__nyanpasu_de>>::deserialize(#deserializer)));
            quote! { if let ::core::result::Result::Ok(value) = #decode { return ::core::result::Result::Ok(#name::#ident(value)); } }
        });
        if !decoded.iter().any(|(candidate, _, _)| *candidate == kind) {
            continue;
        }
        methods.push(quote! {
            #[inline(always)]
            fn #method<__E: #serde::de::Error>(self, value: #ty) -> ::core::result::Result<Self::Value, __E> {
                #(#attempts)*
                ::core::result::Result::Err(__E::custom(#error))
            }
        });
    }
    // An owned String can be moved into the final plain String variant. Other
    // string alternatives are tried by reference first, preserving declaration order.
    let mut owned = Vec::new();
    let mut terminal = false;
    for (kind, ident, field) in &decoded {
        if *kind != Kind::String || terminal {
            continue;
        }
        let field = field.unwrap();
        let ty = field.ty;
        terminal =
            inferred_kind(ty) == Some(Kind::String) && field.attrs.deserialize_with().is_none();
        if terminal {
            owned.push(quote!(return ::core::result::Result::Ok(#name::#ident(value));));
        } else {
            let deserializer =
                quote!(#serde::de::value::StrDeserializer::<#serde::de::value::Error>::new(&value));
            let decode = field.attrs.deserialize_with().map(|path| quote!(#path(#deserializer)))
                .unwrap_or_else(|| quote!(<#ty as #serde::Deserialize<'__nyanpasu_de>>::deserialize(#deserializer)));
            owned.push(quote!(if let ::core::result::Result::Ok(decoded) = #decode { return ::core::result::Result::Ok(#name::#ident(decoded)); }));
        }
    }
    if !owned.is_empty() {
        let tail = (!terminal).then(|| quote!(::core::result::Result::Err(__E::custom(#error))));
        methods.push(quote! {
            #[inline(always)]
            fn visit_string<__E: #serde::de::Error>(self, value: ::std::string::String) -> ::core::result::Result<Self::Value, __E> {
                #(#owned)* #tail
            }
        });
    }
    if let Some((_, ident, _)) = decoded.iter().find(|(kind, _, _)| *kind == Kind::Null) {
        methods.push(quote! {
            #[inline(always)]
            fn visit_unit<__E: #serde::de::Error>(self) -> ::core::result::Result<Self::Value, __E> { ::core::result::Result::Ok(#name::#ident) }
        });
    }
    Ok(quote! {
        const _: () = {
            struct __Visitor #generics (::core::marker::PhantomData<fn() -> (&'__nyanpasu_de (), #original)>) #constraints;
            impl #implementation #serde::de::Visitor<'__nyanpasu_de> for __Visitor #types #constraints {
                type Value = #original;
                #[inline(always)]
                fn expecting(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result { formatter.write_str(#error) }
                #(#methods)*
            }
            impl #implementation #serde::Deserialize<'__nyanpasu_de> for #original #constraints {
                #[inline(always)]
                fn deserialize<__D: #serde::Deserializer<'__nyanpasu_de>>(deserializer: __D) -> ::core::result::Result<Self, __D::Error> {
                    deserializer.deserialize_any(__Visitor :: #types (::core::marker::PhantomData))
                }
            }
        };
    })
}
