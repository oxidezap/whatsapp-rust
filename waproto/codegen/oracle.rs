#![forbid(unsafe_code)]

use crate::emit_owned::{Emitter, ident, member};
use crate::schema::*;
use heck::{ToSnakeCase, ToUpperCamelCase};

impl Emitter<'_> {
    fn oracle_value(&self, value: ValueType, a: &str, b: &str) -> String {
        match value {
            ValueType::Scalar(Scalar::Float | Scalar::Double) => {
                format!("assert_eq!({a}.to_bits(),{b}.to_bits());")
            }
            ValueType::Scalar(Scalar::Bytes) => {
                format!("assert_eq!(AsRef::<[u8]>::as_ref({a}),AsRef::<[u8]>::as_ref({b}));")
            }
            ValueType::Scalar(_) => format!("assert_eq!({a},{b});"),
            ValueType::Enumeration(id) if self.schema.enumeration(id).open => format!(
                "assert_eq!({a}.to_i32(),{b}.to_i32());assert_eq!({a}.is_known(),{b}.is_known());"
            ),
            ValueType::Enumeration(_) => {
                format!("assert_eq!(rt::Enumeration::to_i32({a}),buffa::Enumeration::to_i32({b}));")
            }
            ValueType::Message(id, _) => format!("assert_m{}({a},{b});", id.0),
        }
    }
}

fn oracle_seed(schema: &Schema, id: MessageId, depth: usize) -> Vec<u8> {
    fn varint(mut value: u64, out: &mut Vec<u8>) {
        while value >= 128 {
            out.push((value as u8 & 127) | 128);
            value >>= 7;
        }
        out.push(value as u8);
    }
    let mut out = Vec::new();
    for fid in &schema.message(id).fields {
        let field = schema.field(*fid);
        // Keep one map key across combined fixtures so randomized map
        // iteration cannot obscure the byte-exact codec comparison.
        if schema.message(id).map_entry && field.number.get() == 1 {
            continue;
        }
        let value = match field.shape {
            FieldShape::Singular { value, .. }
            | FieldShape::Repeated { value, .. }
            | FieldShape::Oneof { value, .. } => value,
            FieldShape::Map { entry, .. } => {
                ValueType::Message(entry, MessageEncoding::LengthPrefixed)
            }
        };
        let payload = match value {
            ValueType::Message(child, _) if depth > 0 => oracle_seed(schema, child, depth - 1),
            ValueType::Message(..) => continue,
            ValueType::Scalar(Scalar::String) => b"fixture".to_vec(),
            ValueType::Scalar(Scalar::Bytes) => vec![0x42, 0x81],
            ValueType::Enumeration(id) => {
                let mut payload = Vec::new();
                varint(
                    schema.enumeration(id).values[0].number as i64 as u64,
                    &mut payload,
                );
                payload
            }
            _ => match Emitter::wire(value) {
                0 => vec![1],
                1 => 1f64.to_le_bytes().to_vec(),
                5 => 1f32.to_le_bytes().to_vec(),
                _ => unreachable!(),
            },
        };
        let wire = Emitter::wire(value);
        varint(
            (u64::from(field.number.get()) << 3) | u64::from(wire),
            &mut out,
        );
        if wire == 2 {
            varint(payload.len() as u64, &mut out);
        }
        out.extend_from_slice(&payload);
        if wire == 3 {
            varint((u64::from(field.number.get()) << 3) | 4, &mut out);
        }
    }
    out
}

pub fn generate_oracle(schema: &Schema) -> Result<String, syn::Error> {
    let new = Emitter {
        schema,
        base: "waproto::whatsapp",
    };
    let old = Emitter {
        schema,
        base: "crate::reference",
    };
    let mut source = String::new();
    for (index, message) in schema.messages.iter().enumerate() {
        if message.map_entry {
            continue;
        }
        let id = MessageId(index);
        source.push_str(&format!(
            "fn assert_m{index}(a:&{},b:&{}){{let _=(a,b);",
            new.message_path(id),
            old.message_path(id)
        ));
        for fid in &message.fields {
            let field = schema.field(*fid);
            let slot = member(&field.name);
            match field.shape {
                FieldShape::Singular { value: ValueType::Message(child,_), .. } => source.push_str(&format!("assert_eq!(a.{slot}.is_set(),b.{slot}.is_set());if let(Some(a),Some(b))=(a.{slot}.as_option(),b.{slot}.as_option()){{assert_m{}(a,b);}}",child.0)),
                FieldShape::Singular { value, presence: Presence::Explicit } => source.push_str(&format!("match(&a.{slot},&b.{slot}){{(Some(a),Some(b))=>{{{}}},(None,None)=>{{}},_=>panic!(\"presence mismatch {}.{slot}\"),}}",new.oracle_value(value,"a","b"),message.full_name)),
                FieldShape::Singular { value, .. } => source.push_str(&new.oracle_value(value,&format!("&a.{slot}"),&format!("&b.{slot}"))),
                FieldShape::Repeated { value, .. } => source.push_str(&format!("assert_eq!(a.{slot}.len(),b.{slot}.len());for(a,b)in a.{slot}.iter().zip(&b.{slot}){{{}}}",new.oracle_value(value,"a","b"))),
                FieldShape::Map { value, .. } => source.push_str(&format!("assert_eq!(a.{slot}.len(),b.{slot}.len());for(key,a)in &a.{slot}{{let b=b.{slot}.get(key).expect(\"same map key\");{}}}",new.oracle_value(value,"a","b"))),
                FieldShape::Oneof { .. } => {},
            }
        }
        for oid in &message.oneofs {
            let oneof = schema.oneof(*oid);
            let slot = member(&oneof.name);
            source.push_str(&format!("match(&a.{slot},&b.{slot}){{(None,None)=>{{}},"));
            for fid in &oneof.fields {
                let field = schema.field(*fid);
                let FieldShape::Oneof { value, .. } = field.shape else {
                    unreachable!()
                };
                let variant = ident(&field.name.to_upper_camel_case());
                source.push_str(&format!(
                    "(Some({}::{variant}(a)),Some({}::{variant}(b)))=>{{{}}},",
                    new.oneof_path(*oid),
                    old.oneof_path(*oid),
                    new.oracle_value(value, "a", "b")
                ));
            }
            source.push_str("_=>panic!(\"oneof discriminator mismatch\"),}");
        }
        source.push_str("}\n");
        source.push_str(&format!(
            "#[test]fn m{index}(){{compare::<{},{}>(assert_m{index},check_v{index},&[",
            new.message_path(id),
            old.message_path(id)
        ));
        for fid in &message.fields {
            let field = schema.field(*fid);
            let value = match field.shape {
                FieldShape::Singular { value, .. }
                | FieldShape::Repeated { value, .. }
                | FieldShape::Oneof { value, .. } => value,
                FieldShape::Map { entry, .. } => {
                    ValueType::Message(entry, MessageEncoding::LengthPrefixed)
                }
            };
            let kind = match value {
                ValueType::Message(..) => "Message",
                ValueType::Scalar(Scalar::String) => "String",
                ValueType::Scalar(Scalar::Bytes) => "Bytes",
                _ => match Emitter::wire(value) {
                    0 => "Varint",
                    1 => "Fixed64",
                    5 => "Fixed32",
                    _ => unreachable!(),
                },
            };
            if let ValueType::Message(child, _) = value {
                let seed = oracle_seed(schema, child, 1);
                source.push_str(&format!(
                    "({},Kind::Message(&{seed:?})),",
                    field.number.get()
                ));
            } else {
                source.push_str(&format!("({},Kind::{kind}),", field.number.get()));
            }
        }
        source.push_str("]);}\n");
    }
    source.push_str(&generate_view_oracle(schema));
    let parsed = syn::parse_file(&source)?;
    Ok(prettyplease::unparse(&parsed))
}

fn generate_view_oracle(schema: &Schema) -> String {
    let new = Emitter {
        schema,
        base: "waproto::whatsapp",
    };
    let old = Emitter {
        schema,
        base: "crate::reference",
    };
    let mut out = String::new();
    let value_assert = |value, a: &str, b: &str| {
        new.oracle_value(value, a, b)
            .replace("assert_m", "assert_v")
    };
    for (index, message) in schema.messages.iter().enumerate() {
        if message.map_entry {
            continue;
        }
        let id = MessageId(index);
        out.push_str(&format!(
            "fn assert_v{index}(a:&{}<'_>,b:&{}<'_>){{let _=(a,b);",
            new.view_path(id),
            old.view_path(id)
        ));
        for fid in &message.fields {
            let field = schema.field(*fid);
            let slot = member(&field.name);
            if matches!(
                field.shape,
                FieldShape::Singular {
                    presence: Presence::Required,
                    ..
                }
            ) {
                let accessor = ident(&format!("has_{}", field.name.to_snake_case()));
                out.push_str(&format!("assert_eq!(a.{accessor}(),b.{accessor}());"));
            }
            match field.shape{
            FieldShape::Singular{value:ValueType::Message(child,_),..}=>out.push_str(&format!("assert_eq!(a.{slot}.is_set(),b.{slot}.is_set());if let(Some(a),Some(b))=(a.{slot}.as_option(),b.{slot}.as_option()){{assert_v{}(a,b);}}",child.0)),
            FieldShape::Singular{value,presence:Presence::Explicit}=>out.push_str(&format!("match(&a.{slot},&b.{slot}){{(Some(a),Some(b))=>{{{}}},(None,None)=>{{}},_=>panic!(\"borrowed presence mismatch\"),}}",value_assert(value,"a","b"))),
            FieldShape::Singular{value,..}=>out.push_str(&value_assert(value,&format!("&a.{slot}"),&format!("&b.{slot}"))),
            FieldShape::Repeated{value,..}=>out.push_str(&format!("assert_eq!(a.{slot}.len(),b.{slot}.len());for(a,b)in a.{slot}.iter().zip(b.{slot}.iter()){{{}}}",value_assert(value,"a","b"))),
            FieldShape::Map{key,value,..}=>out.push_str(&format!("assert_eq!(a.{slot}.len(),b.{slot}.len());for((ak,av),(bk,bv))in a.{slot}.iter().zip(b.{slot}.iter()){{{}{}}}",value_assert(ValueType::Scalar(key),"ak","bk"),value_assert(value,"av","bv"))),
            FieldShape::Oneof{..}=>{},
        }
        }
        for oid in &message.oneofs {
            let oneof = schema.oneof(*oid);
            let slot = member(&oneof.name);
            out.push_str(&format!("match(&a.{slot},&b.{slot}){{(None,None)=>{{}},"));
            for fid in &oneof.fields {
                let field = schema.field(*fid);
                let FieldShape::Oneof { value, .. } = field.shape else {
                    unreachable!()
                };
                let variant = ident(&field.name.to_upper_camel_case());
                out.push_str(&format!(
                    "(Some({}::{variant}(a)),Some({}::{variant}(b)))=>{{{}}},",
                    new.view_oneof_path(*oid),
                    old.view_oneof_path(*oid),
                    value_assert(value, "a", "b")
                ));
            }
            out.push_str("_=>panic!(\"borrowed oneof discriminator mismatch\"),}");
        }
        out.push_str("}\n");
        out.push_str(&format!("fn check_v{index}(input:&[u8]){{let a=<{}<'_>as rt::MessageView>::decode_view(input);let b=<{}<'_>as buffa::MessageView>::decode_view(input);match(a,b){{(Ok(a),Ok(b))=>{{assert_v{index}(&a,&b);let ao=rt::MessageView::to_owned_message(&a).expect(\"valid view\");let bo=buffa::MessageView::to_owned_message(&b).expect(\"valid reference view\");assert_m{index}(&ao,&bo);assert_eq!(rt::ViewEncode::encode_to_vec(&a),buffa::ViewEncode::encode_to_vec(&b));let handle=<{} as rt::HasMessageView>::decode_view_handle(bytes::Bytes::copy_from_slice(input)).expect(\"valid handle\");assert_v{index}(handle.view(),&b);assert_m{index}(&handle.to_owned_message(),&bo);let cloned=handle.clone();drop(handle);assert_v{index}(cloned.view(),&b);assert_eq!(cloned.into_bytes().as_ref(),input);}},(Err(a),Err(b))=>assert_eq!(format!(\"{{a:?}}\"),format!(\"{{b:?}}\")),(a,b)=>panic!(\"borrowed rejection mismatch {{a:?}} {{b:?}}\"),}}}}\n",new.view_path(id),old.view_path(id),new.message_path(id)));
    }
    out
}
