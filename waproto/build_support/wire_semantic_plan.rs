//! Bounded semantic visitor plan for the retained Header media projection.
//! Every descriptor field receives a decision, so schema additions cannot
//! silently disappear from the generated visitor's support guard.
use heck::{ToSnakeCase as _, ToUpperCamelCase as _};
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};

use buffa_descriptor::generated::descriptor::{
    DescriptorProto, FileDescriptorSet,
    field_descriptor_proto::{Label, Type},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldMode {
    Ignored,
    Unsupported,
    Bytes,
    Message(u16),
}
#[derive(Debug)]
pub struct Field {
    pub number: i32,
    pub name: String,
    pub string: bool,
    pub oneof_name: Option<String>,
    pub oneof: Option<i32>,
    pub mode: FieldMode,
}
#[derive(Debug)]
pub struct Message {
    pub name: String,
    pub fields: Vec<Field>,
    pub reject_unknowns: bool,
    pub reject_journal: bool,
}
const OWNERS: [&str; 4] = [
    ".whatsapp.Message.InteractiveMessage.Header",
    ".whatsapp.Message.ImageMessage",
    ".whatsapp.ContextInfo",
    ".whatsapp.Message",
];

pub fn plan(fds: &FileDescriptorSet) -> [Message; 4] {
    fn collect<'a>(
        messages: &'a [DescriptorProto],
        scope: &str,
        output: &mut BTreeMap<String, &'a DescriptorProto>,
    ) {
        for message in messages {
            let name = format!("{scope}.{}", message.name.as_deref().expect("message name"));
            output.insert(name.clone(), message);
            collect(&message.nested_type, &name, output);
        }
    }
    let mut descriptors = BTreeMap::new();
    let mut proto2 = true;
    for file in &fds.file {
        if file.package.as_deref() == Some("whatsapp") {
            proto2 &= file
                .syntax
                .as_deref()
                .is_none_or(|syntax| syntax == "proto2");
        }
        collect(
            &file.message_type,
            &format!(".{}", file.package.as_deref().unwrap_or_default()),
            &mut descriptors,
        );
    }
    std::array::from_fn(|index| {
        let owner = descriptors[OWNERS[index]];
        let media = (index == 0)
            .then(|| {
                owner
                    .field
                    .iter()
                    .find(|field| field.number == Some(4))
                    .and_then(|field| field.oneof_index)
            })
            .flatten();
        let fields = owner
            .field
            .iter()
            .map(|field| {
                let number = field.number.expect("field number");
                // Header's ordinary fields are encoded outside the selected
                // oneof projection. Descendant visitors cover their whole owner.
                let ignored = index == 0
                    && media.is_some()
                    && field.oneof_index.is_none()
                    && field.r#type != Some(Type::TYPE_ENUM);
                let candidate = match (index, number) {
                    (0, 4) => FieldMode::Message(1),
                    (0, 6) | (1, 3 | 16) | (3, 1) => FieldMode::Bytes,
                    (1, 17) => FieldMode::Message(2),
                    (2, 3) => FieldMode::Message(3),
                    _ => FieldMode::Unsupported,
                };
                let supported = (index != 0 || (media.is_some() && field.oneof_index == media))
                    && proto2
                    && field.label == Some(Label::LABEL_OPTIONAL)
                    && field.default_value.is_none()
                    && (index == 0 || field.oneof_index.is_none())
                    && match candidate {
                        FieldMode::Bytes => {
                            matches!(field.r#type, Some(Type::TYPE_STRING | Type::TYPE_BYTES))
                        }
                        FieldMode::Message(child) => {
                            field.r#type == Some(Type::TYPE_MESSAGE)
                                && field.type_name.as_deref() == Some(OWNERS[usize::from(child)])
                                && !descriptors[OWNERS[usize::from(child)]]
                                    .options
                                    .as_option()
                                    .is_some_and(|options| options.map_entry == Some(true))
                        }
                        _ => false,
                    };
                Field {
                    number,
                    name: field.name.as_deref().expect("field name").to_owned(),
                    string: field.r#type == Some(Type::TYPE_STRING),
                    oneof_name: field.oneof_index.map(|index| {
                        owner.oneof_decl[index as usize]
                            .name
                            .as_deref()
                            .expect("oneof name")
                            .to_owned()
                    }),
                    oneof: field.oneof_index,
                    mode: if ignored {
                        FieldMode::Ignored
                    } else if supported {
                        candidate
                    } else {
                        FieldMode::Unsupported
                    },
                }
            })
            .collect();
        Message {
            name: OWNERS[index].to_owned(),
            fields,
            reject_unknowns: index != 0,
            reject_journal: index != 0,
        }
    })
}

// This output is compiled by the real-type qualification fixture first. It
// does not become a decode backend merely because its visitors compile.
pub fn emit(messages: &[Message; 4], syntax: &syn::File) -> syn::File {
    fn collect<'a>(
        items: &'a [syn::Item],
        scope: &str,
        output: &mut BTreeMap<String, &'a syn::ItemStruct>,
    ) {
        for item in items {
            match item {
                syn::Item::Struct(owner) => {
                    output.insert(format!("{scope}{}", owner.ident), owner);
                }
                syn::Item::Mod(module) => {
                    if let Some((_, children)) = &module.content {
                        collect(children, &format!("{scope}{}::", module.ident), output);
                    }
                }
                _ => {}
            }
        }
    }
    let mut owners = BTreeMap::new();
    collect(&syntax.items, "", &mut owners);
    let rust_name = |name: &str| {
        let mut parts: Vec<_> = name
            .strip_prefix(".whatsapp.")
            .expect("WhatsApp owner")
            .split('.')
            .collect();
        let leaf = parts.pop().expect("owner name");
        parts
            .iter()
            .map(|part| format!("{}::", part.to_snake_case()))
            .collect::<String>()
            + leaf
    };
    let mut implementations = Vec::new();
    let mut schemas = Vec::new();
    let mut probes = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let name = rust_name(&message.name);
        let owner = owners[&name];
        let path: syn::Path =
            syn::parse_str(&format!("::waproto::whatsapp::{name}")).expect("owner path");
        let parent = name.rsplit_once("::").map_or("", |(parent, _)| parent);
        let mut guards = Vec::new();
        let mut cases = Vec::new();
        let mut schema = Vec::new();
        let mut guarded = BTreeSet::new();
        let member = |name: &str| {
            let name = name.to_snake_case();
            let field = owner
                .fields
                .iter()
                .find(|field| {
                    field
                        .ident
                        .as_ref()
                        .is_some_and(|id| id.to_string().trim_start_matches("r#") == name)
                })
                .expect("descriptor matches generated member");
            (field.ident.as_ref().expect("named field"), field)
        };
        for field in &message.fields {
            let schema_id = index as u16;
            let number = field.number as u32;
            let probe = match field.mode {
                FieldMode::Ignored => quote!(return Ok(false);),
                FieldMode::Unsupported => quote!(return Ok(false);),
                FieldMode::Bytes | FieldMode::Message(_) => {
                    let binding = if matches!(field.mode, FieldMode::Message(_)) {
                        quote!(bytes)
                    } else {
                        quote!(_)
                    };
                    let nested = if let FieldMode::Message(child) = field.mode {
                        quote!(if !probe_node(#child, bytes, ctx.descend()?)? { return Ok(false); })
                    } else {
                        quote!()
                    };
                    quote! {
                        if tag.wire_type() != ::buffa::encoding::WireType::LengthDelimited {
                            return Err(::buffa::DecodeError::WireTypeMismatch { field_number: tag.field_number(), expected: ::buffa::encoding::WireType::LengthDelimited as u8, actual: tag.wire_type() as u8 });
                        }
                        let #binding = ::buffa::types::borrow_bytes(&mut input)?;
                        #nested
                    }
                }
            };
            probes.push(quote!((#schema_id, #number) => { #probe }));
            if field.mode == FieldMode::Ignored {
                continue;
            }
            let number = field.number as u32;
            let (access, ast) = member(field.oneof_name.as_deref().unwrap_or(&field.name));
            if field.mode == FieldMode::Unsupported {
                if field.oneof.is_some() && index == 0 {
                    if !guarded.insert(access.to_string()) {
                        continue;
                    }
                    let same_group: Vec<_> = message
                        .fields
                        .iter()
                        .filter(|candidate| candidate.oneof == field.oneof)
                        .collect();
                    let oneof = format_ident!(
                        "{}",
                        field
                            .oneof_name
                            .as_deref()
                            .expect("oneof name")
                            .to_upper_camel_case()
                    );
                    let enum_path: syn::Path = syn::parse_str(&format!(
                        "::waproto::whatsapp::{}{}{}::{oneof}",
                        parent,
                        if parent.is_empty() { "" } else { "::" },
                        owner.ident.to_string().to_snake_case()
                    ))
                    .expect("oneof path");
                    let alternatives: Vec<_> = same_group
                        .iter()
                        .filter(|candidate| {
                            matches!(candidate.mode, FieldMode::Bytes | FieldMode::Message(_))
                        })
                        .map(|candidate| {
                            let variant = format_ident!("{}", candidate.name.to_upper_camel_case());
                            quote!(Some(#enum_path::#variant(_)))
                        })
                        .collect();
                    guards.push(quote!(
                        matches!(self.#access.as_ref(), None #(| #alternatives)*)
                    ));
                } else {
                    let syn::Type::Path(ty) = &ast.ty else {
                        panic!("generated member type");
                    };
                    let last = &ty.path.segments.last().expect("member type").ident;
                    guards.push(if last == "Option" {
                        quote!(self.#access.is_none())
                    } else if last == "MessageField" {
                        quote!(self.#access.is_unset())
                    } else if last == "Vec" || last == "HashMap" || last == "BTreeMap" {
                        quote!(self.#access.is_empty())
                    } else {
                        quote!(false)
                    });
                }
                continue;
            }
            let value = if field.oneof.is_some() {
                let oneof = format_ident!(
                    "{}",
                    field
                        .oneof_name
                        .as_deref()
                        .expect("oneof name")
                        .to_upper_camel_case()
                );
                let enum_path: syn::Path = syn::parse_str(&format!(
                    "::waproto::whatsapp::{}{}{}::{oneof}",
                    parent,
                    if parent.is_empty() { "" } else { "::" },
                    owner.ident.to_string().to_snake_case()
                ))
                .expect("oneof path");
                let variant = format_ident!("{}", field.name.to_upper_camel_case());
                let value = match field.mode {
                    FieldMode::Message(_) => {
                        quote!(semantic::ValueRef::Child(value.as_ref()))
                    }
                    _ if field.string => quote!(semantic::ValueRef::Bytes(value.as_bytes())),
                    _ => quote!(semantic::ValueRef::Bytes(value.as_ref())),
                };
                quote!(match self.#access.as_ref() { Some(#enum_path::#variant(value)) => Some(#value), _ => None })
            } else {
                match field.mode {
                    FieldMode::Message(_) => {
                        quote!(self.#access.as_option().map(|value| semantic::ValueRef::Child(value)))
                    }
                    _ if field.string => {
                        quote!(self.#access.as_deref().map(|value| semantic::ValueRef::Bytes(value.as_bytes())))
                    }
                    _ => quote!(self.#access.as_deref().map(semantic::ValueRef::Bytes)),
                }
            };
            cases.push(quote!(#number => #value,));
            let kind = match field.mode {
                FieldMode::Message(child) => quote!(semantic::Kind::Message(#child)),
                _ => quote!(semantic::Kind::Bytes),
            };
            let oneof = if field.oneof.is_some() { 1u8 } else { 0u8 };
            schema.push(quote!(semantic::Field { number: #number, kind: #kind, repeated: false, oneof: #oneof }));
        }
        if message.reject_unknowns {
            guards.push(quote!(self.__buffa_unknown_fields.is_empty()));
        }
        if message.reject_journal {
            let storage = owner
                .fields
                .iter()
                .find(|field| {
                    field
                        .ident
                        .as_ref()
                        .is_some_and(|id| id == "__buffa_unknown_fields")
                })
                .expect("unknown storage");
            let syn::Type::Path(ty) = &storage.ty else {
                panic!("storage type");
            };
            if ty
                .path
                .segments
                .iter()
                .any(|segment| segment.ident == "__wire_order")
            {
                guards.push(quote!(!self.__buffa_unknown_fields.active()));
            }
        }
        let support = if guards.is_empty() {
            quote!(true)
        } else {
            quote!(#(#guards)&&*)
        };
        implementations.push(quote! {
            impl semantic::Visitor for #path {
                fn supported(&self) -> bool { #support }
                fn field(&self, number: u32, index: usize) -> Option<semantic::ValueRef<'_>> {
                    if index != 0 { return None; }
                    match number { #(#cases)* _ => None }
                }
            }
        });
        schemas.push(quote!(&[#(#schema),*]));
    }
    syn::parse2(quote! {
        const SCHEMA: semantic::Schema = &[#(#schemas),*];
        #(#implementations)*
        // Probe a captured known projection record before the typed decoder
        // mutates its owner. Rejection allocates nothing and debits no budget.
        fn can_stage_known(record: &[u8], ctx: ::buffa::DecodeContext<'_>) -> Result<bool, ::buffa::DecodeError> {
            probe_node(0, record, ctx)
        }
        fn probe_node(schema: u16, mut input: &[u8], ctx: ::buffa::DecodeContext<'_>) -> Result<bool, ::buffa::DecodeError> {
            while !input.is_empty() {
                let tag = ::buffa::encoding::Tag::decode(&mut input)?;
                match (schema, tag.field_number()) { #(#probes),* _ => return Ok(false) }
            }
            Ok(true)
        }
    }).expect("visitor syntax")
}
