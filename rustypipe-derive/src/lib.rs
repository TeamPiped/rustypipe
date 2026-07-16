//! Procedural macros for rustypipe.
//!
//! Currently provides:
//! - [`ytq`]: builds a [`rustypipe::json::Query`] from a compact path
//!   expression at compile time.
//! - [`FromYtNode`]: derive macro that generates
//!   `crate::json::FromYtNode::from_node` and (when `Deserialize` is needed)
//!   the `Deserialize` impl that goes through `from_node`.
//!
//! `ytq!` syntax:
//! - `.key` — key access
//! - `[index]` — array index
//! - `||` — top-level alternation (e.g. `.a || .b`)
//! - `.(.a || .b)` — sub-path alternation; expands to a cross-product
//!   (e.g. `.prefix.(.a || .b).suffix` becomes `.prefix.a.suffix` and
//!   `.prefix.b.suffix`)
//! - `$root` — inside a `.(...)` group, represents the empty path. Useful when
//!   one of the alternatives should be the root itself (e.g.
//!   `($root || .continuationEndpoint).continuationCommand.token` expands to
//!   `.continuationCommand.token` and `.continuationEndpoint.continuationCommand.token`)

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    DeriveInput, LitInt, Token,
};

/// A single step in a path: either a key access (`.key`) or an index
/// (`[idx]`).
#[derive(Clone, Debug)]
enum Step {
    Key(String),
    Index(usize),
}

/// A sub-path: an ordered list of [`Step`]s.
#[derive(Clone, Debug, Default)]
struct SubPath {
    steps: Vec<Step>,
}

/// A path expression parsed as a sequence of items. Each item is either
/// a list of steps (`SubPath`) or a group (`.(.a || .b)`) that expands
/// into multiple sub-paths.
#[derive(Clone, Debug)]
enum PathItem {
    Sub(SubPath),
    Group(Vec<SubPath>),
}

/// A complete path expression: a list of top-level `||`-separated items.
#[derive(Clone, Debug)]
struct QueryPath {
    items: Vec<PathItem>,
}

impl Parse for QueryPath {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut items = Vec::new();
        items.push(parse_path_seq(input)?);
        while input.peek(Token![||]) {
            let _: Token![||] = input.parse()?;
            items.push(parse_path_seq(input)?);
        }
        Ok(QueryPath { items })
    }
}

/// Parse a sequence of steps and groups (a single `||`-separated branch).
/// Returns a `PathItem` which is either a `SubPath` (simple path) or a
/// `Group` (list of sub-paths, one per option in a `.(...)` group, with
/// any surrounding steps prepended/appended to each).
fn parse_path_seq(input: ParseStream) -> syn::Result<PathItem> {
    // The current "branches" being built. Starts with a single empty branch.
    let mut branches: Vec<SubPath> = vec![SubPath::default()];

    loop {
        if input.peek(Token![||]) {
            break;
        }
        if input.peek(Token![.]) {
            let fork = input.fork();
            let _: Token![.] = fork.parse()?;
            if fork.peek(syn::token::Paren) {
                // It's a `.(.a || .b)` group.
                let _: Token![.] = input.parse()?;
                let content;
                let _ = syn::parenthesized!(content in input);
                let group = parse_group(&content)?;
                // For each existing branch, create one new branch per option,
                // extending the branch with the option's steps.
                let mut new_branches: Vec<SubPath> = Vec::new();
                for b in branches.iter() {
                    for opt in &group {
                        let mut nb = b.clone();
                        nb.steps.extend(opt.steps.iter().cloned());
                        new_branches.push(nb);
                    }
                }
                branches = new_branches;
            } else {
                // It's a `.key` access. Apply to all branches.
                let _: Token![.] = input.parse()?;
                let key = if input.peek(syn::LitStr) {
                    let lit: syn::LitStr = input.parse()?;
                    Step::Key(lit.value())
                } else {
                    let ident: syn::Ident = input.parse()?;
                    Step::Key(ident.to_string())
                };
                for b in branches.iter_mut() {
                    b.steps.push(key.clone());
                }
            }
        } else if input.peek(syn::token::Paren) {
            // It's a `(.a || .b)` group (the leading `.` is omitted).
            // The first option in the group must be `$root` or start with `.`.
            let content;
            let _ = syn::parenthesized!(content in input);
            let group = parse_group(&content)?;
            let mut new_branches: Vec<SubPath> = Vec::new();
            for b in branches.iter() {
                for opt in &group {
                    let mut nb = b.clone();
                    nb.steps.extend(opt.steps.iter().cloned());
                    new_branches.push(nb);
                }
            }
            branches = new_branches;
        } else if input.peek(syn::token::Bracket) {
            let content;
            let _ = syn::bracketed!(content in input);
            let idx: LitInt = content.parse()?;
            let value: usize = idx.base10_parse()?;
            let index = Step::Index(value);
            for b in branches.iter_mut() {
                b.steps.push(index.clone());
            }
        } else {
            break;
        }
    }

    if branches.len() == 1 {
        Ok(PathItem::Sub(branches.into_iter().next().unwrap()))
    } else {
        Ok(PathItem::Group(branches))
    }
}

/// Parse the inside of a `.(...)` group: a list of sub-paths separated by
/// `||`. Each sub-path is a sequence of steps.
fn parse_group(input: ParseStream) -> syn::Result<Vec<SubPath>> {
    let mut subs = Vec::new();
    subs.push(parse_subpath(input)?);
    while input.peek(Token![||]) {
        let _: Token![||] = input.parse()?;
        subs.push(parse_subpath(input)?);
    }
    Ok(subs)
}

/// Parse a single sub-path inside a group: a sequence of `.key`/`[idx]`
/// steps (no leading `.` required, no group support). A bare `$root` is
/// also accepted and represents the empty path (i.e. the group option
/// contributes no steps).
fn parse_subpath(input: ParseStream) -> syn::Result<SubPath> {
    // Bare `$root` is allowed inside a group and means "no steps".
    if input.peek(Token![$]) {
        let _: Token![$] = input.parse()?;
        let ident: syn::Ident = input.parse()?;
        if ident == "root" {
            return Ok(SubPath { steps: Vec::new() });
        }
        return Err(syn::Error::new(
            ident.span(),
            "expected `root` after `$` (only `$root` is supported)",
        ));
    }
    let mut steps: Vec<Step> = Vec::new();
    loop {
        if input.peek(Token![.]) {
            let _: Token![.] = input.parse()?;
            let key = if input.peek(syn::LitStr) {
                let lit: syn::LitStr = input.parse()?;
                Step::Key(lit.value())
            } else {
                let ident: syn::Ident = input.parse()?;
                Step::Key(ident.to_string())
            };
            steps.push(key);
        } else if input.peek(syn::token::Bracket) {
            let content;
            let _ = syn::bracketed!(content in input);
            let idx: LitInt = content.parse()?;
            let value: usize = idx.base10_parse()?;
            steps.push(Step::Index(value));
        } else {
            break;
        }
    }
    if steps.is_empty() {
        Err(syn::Error::new(input.span(), "expected a sub-path step or `$root`"))
    } else {
        Ok(SubPath { steps })
    }
}

/// The `ytq!` proc-macro: builds a `Query::first_of(&[...])` expression at
/// compile time from a compact path expression.
#[proc_macro]
pub fn ytq(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let path: QueryPath = match syn::parse2(input.into()) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error().into(),
    };

    // Expand to a flat list of branches (each branch is a SubPath).
    let branches = expand_branches(&path.items);

    // Generate the `PathSegment` expressions for each branch.
    // We use `crate::` (not `$crate::`) because the proc-macro output runs
    // in the calling crate's context, and `crate::` resolves to the current
    // crate at the call site. The types are `pub(crate)` so they're visible
    // from within the `rustypipe` crate.
    let branch_tokens: Vec<TokenStream> = branches
        .iter()
        .map(|branch| {
            let segs: Vec<TokenStream> = branch
                .steps
                .iter()
                .map(|step| match step {
                    Step::Key(k) => quote! {
                        crate::json::PathSegment::Key(#k)
                    },
                    Step::Index(i) => quote! {
                        crate::json::PathSegment::Index(#i)
                    },
                })
                .collect();
            quote! {
                &[#(#segs),*]
            }
        })
        .collect();

    quote! {
        crate::json::Query::first_of(&[ #(#branch_tokens),* ])
    }
    .into()
}

/// Expand a list of top-level `||`-separated items into a flat list of
/// branches. Each `PathItem` is a separate top-level branch (or set of
/// branches for a `Group`). A `Group` expands into one branch per option.
fn expand_branches(items: &[PathItem]) -> Vec<SubPath> {
    let mut branches: Vec<SubPath> = Vec::new();
    for item in items {
        match item {
            PathItem::Sub(sub) => {
                branches.push(sub.clone());
            }
            PathItem::Group(options) => {
                for opt in options {
                    branches.push(opt.clone());
                }
            }
        }
    }
    branches
}

// ---------------------------------------------------------------------------
// #[derive(FromYtNode)]
// ---------------------------------------------------------------------------

/// Field-level attributes supported by `#[derive(FromYtNode)]`.
#[derive(Clone, Debug, Default)]
struct FieldAttrs {
    /// Explicit ytq path (multiple paths joined by `||`), stored as the
    /// raw token stream captured from the attribute. Used as the input
    /// to `ytq!` in the generated code.
    path: Option<TokenStream>,
    /// `ytq_text`: this field should be a `String` resolved via `yt_text`.
    text: bool,
    /// `ytq_thumb`: this field should be a `Vec<Thumbnail>` resolved via
    /// `yt_thumbnails`.
    thumb: bool,
    /// `ytq_lossy`: this field should be a `Vec<T>` resolved via
    /// `deserialize_items_lossy`.
    lossy: bool,
    /// `ytq_default`: a missing path or value should fall back to
    /// `Default::default()` rather than failing the whole `from_node`.
    default: bool,
    /// `ytq_enum`: this field is a string-keyed enum; the value is resolved as
    /// a string and converted via `<Self as FromStr>::from_str`.
    is_enum: bool,
    /// `ytq_attributed_text`: this field is a `TextComponents` resolved via
    /// `AttributedText::from_node`.
    attributed_text: bool,
}

impl FieldAttrs {
    fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut out = FieldAttrs::default();
        for attr in attrs {
            if !attr.path().is_ident("ytq")
                && !attr.path().is_ident("ytq_text")
                && !attr.path().is_ident("ytq_thumb")
                && !attr.path().is_ident("ytq_lossy")
                && !attr.path().is_ident("ytq_default")
                && !attr.path().is_ident("ytq_enum")
                && !attr.path().is_ident("ytq_attributed_text")
            {
                continue;
            }
            if attr.path().is_ident("ytq") {
                // `#[ytq(.a.b || .c)]` — capture the raw tokens inside the
                // parens and pass them to `ytq!` in the generated code. We
                // extract just the inner tokens by skipping the outer path
                // and parens via `parse_args`.
                let tokens: TokenStream = attr.parse_args()?;
                out.path = Some(tokens);
            } else if attr.path().is_ident("ytq_text") {
                out.text = true;
            } else if attr.path().is_ident("ytq_thumb") {
                out.thumb = true;
            } else if attr.path().is_ident("ytq_lossy") {
                out.lossy = true;
            } else if attr.path().is_ident("ytq_default") {
                out.default = true;
            } else if attr.path().is_ident("ytq_enum") {
                out.is_enum = true;
            } else if attr.path().is_ident("ytq_attributed_text") {
                out.attributed_text = true;
            }
        }
        Ok(out)
    }
}

/// Decide the kind of expression to emit for a field, based on its attributes
/// and its Rust type.
enum FieldKind {
    /// `node.query(q).and_then(|n| n.text())` (or `node.text_at(q)`)
    Text,
    /// `node.query(q).map(yt_thumbnails).unwrap_or_default()`
    Thumbnail,
    /// `node.deserialize_items_lossy` on the queried sub-node
    Lossy,
    /// `node.deserialize::<T>()` on the queried sub-node
    Nested,
    /// `<Enum>::from_str(&node.query_str(q)?)`
    Enum,
    /// `AttributedText::from_node(&node.query(q)?)`
    AttributedText,
    /// `node.query_str(q)`
    String,
    /// `node.query_u32(q)`
    U32,
    /// `node.query_u64(q)`
    U64,
    /// `node.as_bool()` (from the queried sub-node)
    Bool,
    /// `Option<T>` wrapper that returns `None` on miss.
    Optional(Box<FieldKind>),
}

/// Resolved type-driven `FieldKind` plus the inner type token (for
/// `Vec<T>` / `Option<T>` cases).
struct ResolvedField {
    kind: FieldKind,
    /// For `Vec<T>` / `Option<T>`, the inner `T` token; `None` otherwise.
    inner: Option<syn::Type>,
}

impl FieldKind {
    fn from_type(ty: &syn::Type, attrs: &FieldAttrs) -> ResolvedField {
        // Check Option<T> first so we can propagate attributes to the inner
        // type (e.g. `Option<String>` with `#[ytq_text]` -> `Optional(Text)`).
        let type_str = quote!(#ty).to_string();
        let type_str = type_str.replace(' ', "");

        if let Some(inner_str) = type_str
            .strip_prefix("Option<")
            .and_then(|s| s.strip_suffix('>'))
        {
            let inner: syn::Type = syn::parse_str(inner_str).unwrap_or_else(|_| {
                syn::parse_str("String").unwrap()
            });
            let inner_resolved = FieldKind::from_type(&inner, attrs);
            ResolvedField {
                kind: FieldKind::Optional(Box::new(inner_resolved.kind)),
                inner: Some(inner),
            }
        } else if attrs.text {
            ResolvedField { kind: FieldKind::Text, inner: None }
        } else if attrs.thumb {
            ResolvedField { kind: FieldKind::Thumbnail, inner: None }
        } else if attrs.lossy {
            // For `Vec<T>` with `ytq_lossy`, extract the inner T.
            if let Some(inner_str) = type_str
                .strip_prefix("Vec<")
                .and_then(|s| s.strip_suffix('>'))
            {
                let inner: syn::Type = syn::parse_str(inner_str).unwrap_or_else(|_| {
                    syn::parse_str("String").unwrap()
                });
                ResolvedField { kind: FieldKind::Lossy, inner: Some(inner) }
            } else {
                ResolvedField { kind: FieldKind::Lossy, inner: None }
            }
        } else if attrs.is_enum {
            ResolvedField { kind: FieldKind::Enum, inner: None }
        } else if attrs.attributed_text {
            ResolvedField {
                kind: FieldKind::AttributedText,
                inner: None,
            }
        } else if type_str == "String" {
            ResolvedField { kind: FieldKind::String, inner: None }
        } else if type_str == "u32" {
            ResolvedField { kind: FieldKind::U32, inner: None }
        } else if type_str == "u64" {
            ResolvedField { kind: FieldKind::U64, inner: None }
        } else if type_str == "bool" {
            ResolvedField { kind: FieldKind::Bool, inner: None }
        } else if let Some(inner_str) = type_str
            .strip_prefix("Vec<")
            .and_then(|s| s.strip_suffix('>'))
        {
            let inner: syn::Type = syn::parse_str(inner_str).unwrap_or_else(|_| {
                syn::parse_str("String").unwrap()
            });
            ResolvedField { kind: FieldKind::Lossy, inner: Some(inner) }
        } else {
            ResolvedField { kind: FieldKind::Nested, inner: None }
        }
    }
}

/// Derive `FromYtNode<'a>` for a struct. Generates a `from_node` body that
/// walks `ytq!` paths for each field and dispatches on the field's type or
/// explicit attribute (`ytq_text`, `ytq_thumb`, `ytq_lossy`, `ytq_default`,
/// `ytq_enum`, `ytq_attributed_text`).
#[proc_macro_derive(
    FromYtNode,
    attributes(
        ytq,
        ytq_text,
        ytq_thumb,
        ytq_lossy,
        ytq_default,
        ytq_enum,
        ytq_attributed_text
    )
)]
pub fn derive_from_yt_node(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: DeriveInput = match syn::parse2(input.into()) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error().into(),
    };

    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    // Add a synthetic `'a` lifetime parameter (used by `JsonNode<'a>`) if the
    // struct's own generics don't already include it.
    let lifetime_a_lifetime: syn::Lifetime =
        syn::parse_quote!('__yt_node_a);
    let lifetime_a_param: syn::GenericParam = syn::parse_quote!('__yt_node_a);
    let mut augmented_generics = input.generics.clone();
    let has_a = augmented_generics
        .lifetimes()
        .any(|l| l.lifetime.ident == "a");
    if !has_a {
        augmented_generics
            .params
            .insert(0, lifetime_a_param);
    }
    let (impl_generics_aug, _, _) = augmented_generics.split_for_impl();
    let lifetime_a = lifetime_a_lifetime;

    let fields_named = match &input.data {
        syn::Data::Struct(s) => match &s.fields {
            syn::Fields::Named(named) => named.clone(),
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "FromYtNode only supports structs with named fields",
                )
                .to_compile_error()
                .into()
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "FromYtNode only supports structs")
                .to_compile_error()
                .into()
        }
    };
    let fields = &fields_named.named;

    let mut field_exprs: Vec<TokenStream> = Vec::new();
    for field in fields.iter() {
        let field_name = field.ident.as_ref().expect("named field");
        let field_type = &field.ty;
        let attrs = match FieldAttrs::from_attrs(&field.attrs) {
            Ok(a) => a,
            Err(e) => return e.to_compile_error().into(),
        };
        let path = attrs.path.clone().unwrap_or_else(|| {
            // No explicit `#[ytq(...)]` attribute — infer the field name as
            // the path (camelCase, e.g. `video_id` -> `.videoId`).
            let cc = camel_case(field_name);
            cc.parse::<TokenStream>().unwrap_or_default()
        });

        let resolved = FieldKind::from_type(field_type, &attrs);
        let kind = resolved.kind;
        let inner_ty = resolved.inner.as_ref();

        let expr = match kind {
            FieldKind::Text => {
                if attrs.default {
                    quote! {
                        crate::json::JsonNode::text_at(&node, crate::json::ytq!(#path)).unwrap_or_default()
                    }
                } else {
                    quote! {
                        match crate::json::JsonNode::text_at(&node, crate::json::ytq!(#path)) {
                            Some(s) => s,
                            None => {
                                if !swallow {
                                    return None;
                                }
                                String::new()
                            }
                        }
                    }
                }
            }
            FieldKind::Thumbnail => quote! {
                node.query_thumbnails(crate::json::ytq!(#path))
            },
            FieldKind::Lossy => {
                let item_ty = inner_ty.cloned().unwrap_or_else(|| syn::parse_quote!(#field_type));
                quote! {
                    {
                        let (items, mut warnings) = match node.query(crate::json::ytq!(#path)) {
                            Some(n) => n.deserialize_items_lossy::<#item_ty>(),
                            None => (Vec::new(), Vec::new()),
                        };
                        for w in warnings {
                            node_warnings.push(w);
                        }
                        items
                    }
                }
            }
            FieldKind::Nested => quote! {
                match node.query(crate::json::ytq!(#path)) {
                    Some(n) => match n.deserialize::<#field_type>() {
                        Ok(v) => v,
                        Err(e) => {
                            if !swallow {
                                return None;
                            }
                            node_warnings.push(e.to_string());
                            <#field_type as Default>::default()
                        }
                    },
                    None => {
                        if !swallow {
                            return None;
                        }
                        <#field_type as Default>::default()
                    }
                }
            },
            FieldKind::Enum => quote! {
                match crate::json::JsonNode::query_str(&node, crate::json::ytq!(#path)) {
                    Some(s) => match <#field_type as std::str::FromStr>::from_str(&s) {
                        Ok(v) => v,
                        Err(e) => {
                            if !swallow {
                                return None;
                            }
                            node_warnings.push(e);
                            <#field_type as Default>::default()
                        }
                    },
                    None => {
                        if !swallow {
                            return None;
                        }
                        <#field_type as Default>::default()
                    }
                }
            },
            FieldKind::AttributedText => quote! {{
                let path = crate::json::ytq!(#path);
                match node.query(path) {
                    Some(q) => {
                        let components = crate::serializer::text::AttributedText::from_node(&q)
                            .unwrap_or_default();
                        <#field_type as From<crate::serializer::text::TextComponents>>::from(components)
                    }
                    None => {
                        if !swallow {
                            return None;
                        }
                        <#field_type as Default>::default()
                    }
                }
            }},
            FieldKind::String => {
                if attrs.default {
                    quote! {
                        crate::json::JsonNode::query_str(&node, crate::json::ytq!(#path)).unwrap_or_default()
                    }
                } else {
                    quote! {
                        match crate::json::JsonNode::query_str(&node, crate::json::ytq!(#path)) {
                            Some(s) => s,
                            None => {
                                if !swallow {
                                    return None;
                                }
                                String::new()
                            }
                        }
                    }
                }
            }
            FieldKind::U32 => {
                if attrs.default {
                    quote! {
                        crate::json::JsonNode::query_u32(&node, crate::json::ytq!(#path)).unwrap_or_default()
                    }
                } else {
                    quote! {
                        match crate::json::JsonNode::query_u32(&node, crate::json::ytq!(#path)) {
                            Some(v) => v,
                            None => {
                                if !swallow {
                                    return None;
                                }
                                0u32
                            }
                        }
                    }
                }
            }
            FieldKind::U64 => {
                if attrs.default {
                    quote! {
                        crate::json::JsonNode::query_u64(&node, crate::json::ytq!(#path)).unwrap_or_default()
                    }
                } else {
                    quote! {
                        match crate::json::JsonNode::query_u64(&node, crate::json::ytq!(#path)) {
                            Some(v) => v,
                            None => {
                                if !swallow {
                                    return None;
                                }
                                0u64
                            }
                        }
                    }
                }
            }
            FieldKind::Bool => {
                if attrs.default {
                    quote! {
                        node.query(crate::json::ytq!(#path))
                            .and_then(|n| n.as_bool())
                            .unwrap_or(false)
                    }
                } else {
                    quote! {
                        match node.query(crate::json::ytq!(#path)) {
                            Some(n) => match n.as_bool() {
                                Some(v) => v,
                                None => {
                                    if !swallow {
                                        return None;
                                    }
                                    false
                                }
                            },
                            None => {
                                if !swallow {
                                    return None;
                                }
                                false
                            }
                        }
                    }
                }
            }
            FieldKind::Optional(inner_kind) => {
                // For `Option<T>`, return `None` if the path is missing.
                let inner_expr: TokenStream = match &*inner_kind {
                    FieldKind::String => quote! {
                        crate::json::JsonNode::query_str(&node, crate::json::ytq!(#path))
                    },
                    FieldKind::U32 => quote! {
                        crate::json::JsonNode::query_u32(&node, crate::json::ytq!(#path))
                    },
                    FieldKind::U64 => quote! {
                        crate::json::JsonNode::query_u64(&node, crate::json::ytq!(#path))
                    },
                    FieldKind::Bool => quote! {
                        node.query(crate::json::ytq!(#path))
                            .and_then(|n| n.as_bool())
                    },
                    FieldKind::Text => quote! {
                        crate::json::JsonNode::text_at(&node, crate::json::ytq!(#path))
                    },
                    FieldKind::AttributedText => {
                        // For `Option<String>` with AttributedText, convert
                        // TextComponents to String via the first component's
                        // text.
                        quote! {
                            node.query(crate::json::ytq!(#path))
                                .and_then(|q| crate::serializer::text::AttributedText::from_node(&q))
                                .map(|tc| {
                                    tc.0.into_iter()
                                        .next()
                                        .map(|c| c.into_string())
                                        .unwrap_or_default()
                                })
                        }
                    }
                    FieldKind::Nested => {
                        // For `Option<T>`, try to deserialize the inner type
                        // from the queried sub-node. For `Option<JsonValue>`
                        // this works because `JsonValue: DeserializeOwned`.
                        let inner_ty = inner_ty.cloned()
                            .unwrap_or_else(|| syn::parse_quote!(#field_type));
                        quote! {
                            match node.query(crate::json::ytq!(#path)) {
                                Some(n) => match n.deserialize::<#inner_ty>() {
                                    Ok(v) => Some(v),
                                    Err(_) => None,
                                },
                                None => None,
                            }
                        }
                    }
                    _ => quote! { None },
                };
                quote! { #inner_expr }
            }
        };

        field_exprs.push(quote! {
            #field_name: #expr,
        });
    }

    let swallow = quote! { swallow };
    let default_handler = if attrs_default_present(&fields_named) {
        quote! { true }
    } else {
        quote! { false }
    };

    let expanded = quote! {
        impl #impl_generics_aug crate::json::FromYtNode<#lifetime_a> for #ident #ty_generics #where_clause {
            fn from_node(node: &crate::json::JsonNode<#lifetime_a>) -> Option<Self> {
                let mut node_warnings: Vec<String> = Vec::new();
                let swallow = #default_handler;
                Some(Self {
                    #(#field_exprs)*
                })
            }
        }

        impl<'de> serde::Deserialize<'de> for #ident {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = crate::json::JsonValue::deserialize(deserializer)?;
                crate::json::round_trip(&value, |root| <Self as crate::json::FromYtNode>::from_node(root))
                    .ok_or_else(|| serde::de::Error::custom(
                        concat!("failed to deserialize ", stringify!(#ident), " from ytq! node")
                    ))
            }
        }
    };

    expanded.into()
}

fn attrs_default_present(fields: &syn::FieldsNamed) -> bool {
    fields.named.iter().any(|f| {
        f.attrs
            .iter()
            .any(|a| a.path().is_ident("ytq_default"))
    })
}

/// Convert a snake_case field name into a camelCase ytq-style path component
/// (e.g. `video_id` -> `.videoId`, `id` -> `.id`).
fn camel_case(name: &syn::Ident) -> String {
    let s = name.to_string();
    let parts: Vec<&str> = s.split('_').collect();
    if parts.is_empty() {
        return format!(".{s}");
    }
    let mut out = String::new();
    out.push('.');
    out.push_str(parts[0]);
    for part in &parts[1..] {
        if part.is_empty() {
            continue;
        }
        let mut chars = part.chars();
        if let Some(c) = chars.next() {
            out.push(c.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    out
}
