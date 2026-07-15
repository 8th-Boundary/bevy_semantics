use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Attribute, DeriveInput, Error, Ident, LitStr, Result, Token, Type};

const KIND_DOMAIN: &[u8] = b"bevy_semantics.kind.v1:";

#[proc_macro]
pub fn __kind_raw(input: TokenStream) -> TokenStream {
    let name = parse_macro_input!(input as LitStr);
    match canonical_kind(&name) {
        Ok((_, raw)) => quote!(#raw).into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(SemanticComponent, attributes(semantic))]
pub fn derive_semantic_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_derive(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Implement `SemanticComponent` for a concrete type, including a concrete
/// instantiation of a generic component type.
#[proc_macro]
pub fn semantic_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ConcreteSemanticComponent);
    match expand_concrete(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_derive(input: &DeriveInput) -> Result<proc_macro2::TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "generic semantic components require an explicit concrete implementation; use `semantic_component!(Container<Concrete>, kind = \"Container<Concrete>\")`",
        ));
    }

    let crate_path = semantics_crate_path(input.ident.span())?;
    let name = parse_semantic_name(&input.attrs, input.ident.span())?;
    let ident = &input.ident;
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #crate_path::SemanticComponent for #ident #ty_generics #where_clause {
            const KIND: #crate_path::Kind = #crate_path::kind!(#name);
            const KIND_NAME: &'static str = #name;
        }
    })
}

struct ConcreteSemanticComponent {
    ty: Type,
    name: LitStr,
}

impl Parse for ConcreteSemanticComponent {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let ty = input.parse()?;
        input.parse::<Token![,]>()?;
        let key: Ident = input.parse()?;
        if key != "kind" {
            return Err(Error::new(
                key.span(),
                "expected `kind = \"CanonicalName\"`",
            ));
        }
        input.parse::<Token![=]>()?;
        let name = input.parse()?;
        let _ = input.parse::<Option<Token![,]>>()?;
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after semantic component declaration"));
        }
        Ok(Self { ty, name })
    }
}

fn expand_concrete(input: ConcreteSemanticComponent) -> Result<proc_macro2::TokenStream> {
    let crate_path = semantics_crate_path(input.name.span())?;
    let (canonical, _) = canonical_kind(&input.name)?;
    let name = LitStr::new(&canonical, input.name.span());
    let ty = input.ty;

    Ok(quote! {
        impl #crate_path::SemanticComponent for #ty {
            const KIND: #crate_path::Kind = #crate_path::kind!(#name);
            const KIND_NAME: &'static str = #name;
        }
    })
}

fn parse_semantic_name(attrs: &[Attribute], fallback_span: proc_macro2::Span) -> Result<LitStr> {
    let semantic = attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("semantic"))
        .collect::<Vec<_>>();
    let [attribute] = semantic.as_slice() else {
        let message = if semantic.is_empty() {
            "missing `#[semantic(kind = \"CanonicalName\")]` attribute"
        } else {
            "duplicate `semantic` attributes"
        };
        return Err(match semantic.first().copied() {
            Some(attribute) => Error::new_spanned(attribute, message),
            None => Error::new(fallback_span, message),
        });
    };

    let mut name: Option<LitStr> = None;
    attribute.parse_nested_meta(|meta| {
        if !meta.path.is_ident("kind") {
            return Err(meta.error("unknown semantic key; expected `kind`"));
        }
        if name.is_some() {
            return Err(meta.error("duplicate `kind` key"));
        }
        name = Some(meta.value()?.parse()?);
        Ok(())
    })?;

    let name = name.ok_or_else(|| {
        Error::new_spanned(attribute, "missing `kind = \"CanonicalName\"` argument")
    })?;
    let (canonical, _) = canonical_kind(&name)?;
    Ok(LitStr::new(&canonical, name.span()))
}

fn canonical_kind(name: &LitStr) -> Result<(String, u64)> {
    let value = name.value();
    let canonical = value.trim();
    if canonical.is_empty() {
        return Err(Error::new(
            name.span(),
            "semantic kind name cannot be empty",
        ));
    }

    // This is the compile-time half of the stable Kind identity protocol.
    // Keep it identical to `bevy_semantics::registry::hash_canonical_name`.
    let mut hasher = blake3::Hasher::new();
    hasher.update(KIND_DOMAIN);
    hasher.update(canonical.as_bytes());
    let digest = hasher.finalize();
    let mut raw = [0_u8; 8];
    raw.copy_from_slice(&digest.as_bytes()[..8]);
    Ok((canonical.to_owned(), u64::from_le_bytes(raw)))
}

fn semantics_crate_path(span: proc_macro2::Span) -> Result<proc_macro2::TokenStream> {
    match crate_name("bevy_semantics").map_err(|error| Error::new(span, error))? {
        FoundCrate::Itself => Ok(quote!(::bevy_semantics)),
        FoundCrate::Name(name) => {
            let ident = format_ident!("{}", name);
            Ok(quote!(::#ident))
        }
    }
}
