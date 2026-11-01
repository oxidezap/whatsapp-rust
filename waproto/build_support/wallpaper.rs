//! Retain the two historical string fields alongside the new fixed32 fields.
//! These are distinguished by (field number, wire type), never by conversion.
use buffa_descriptor::generated::descriptor::{
    FieldDescriptorProto, FileDescriptorSet,
    field_descriptor_proto::{Label, Type},
};
use std::io;
use syn::visit_mut::{self, VisitMut};

pub(super) fn retain_fields(fds: &mut FileDescriptorSet) -> io::Result<()> {
    let message = fds
        .file
        .iter_mut()
        .filter(|f| f.package.as_deref() == Some("whatsapp"))
        .flat_map(|f| f.message_type.iter_mut())
        .find(|m| m.name.as_deref() == Some("Message"))
        .and_then(|m| {
            m.nested_type
                .iter_mut()
                .find(|m| m.name.as_deref() == Some("ChatSolidColorWallpaper"))
        })
        .ok_or_else(|| io::Error::other("wallpaper compatibility target missing"))?;
    for (number, current, legacy) in [
        (1, "backgroundLightArgb", "colorLight"),
        (2, "backgroundDarkArgb", "colorDark"),
    ] {
        let fields: Vec<_> = message
            .field
            .iter()
            .filter(|f| f.number == Some(number))
            .collect();
        if fields.len() != 1
            || fields[0].name.as_deref() != Some(current)
            || fields[0].r#type != Some(Type::TYPE_FIXED32)
            || fields[0].label != Some(Label::LABEL_OPTIONAL)
            || message
                .field
                .iter()
                .any(|f| f.name.as_deref() == Some(legacy))
        {
            return Err(io::Error::other(
                "wallpaper wire-type compatibility requires source review",
            ));
        }
        message.field.push(FieldDescriptorProto {
            name: Some(legacy.into()),
            json_name: Some(legacy.into()),
            number: Some(number),
            r#type: Some(Type::TYPE_STRING),
            label: Some(Label::LABEL_OPTIONAL),
            ..Default::default()
        });
    }
    Ok(())
}

#[derive(Default)]
struct WireChecks {
    values: Vec<syn::Expr>,
    invalid: bool,
}
impl VisitMut for WireChecks {
    fn visit_expr_call_mut(&mut self, call: &mut syn::ExprCall) {
        if let syn::Expr::Path(function) = call.func.as_ref()
            && function
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                == ["buffa", "encoding", "check_wire_type"]
        {
            let tag = call
                .args
                .first()
                .is_some_and(|arg| matches!(arg, syn::Expr::Path(p) if p.path.is_ident("tag")));
            let wire = call.args.iter().nth(1).filter(|arg| {
                let syn::Expr::Path(path) = arg else { return false; };
                let names=path.path.segments.iter().map(|s|s.ident.to_string()).collect::<Vec<_>>();
                matches!(names.as_slice(), [a,b,c,d] if a=="buffa" && b=="encoding" && c=="WireType" && matches!(d.as_str(),"Fixed32"|"LengthDelimited"))
            });
            if tag
                && call.args.len() == 2
                && let Some(wire) = wire
            {
                self.values.push(wire.clone());
            } else {
                self.invalid = true;
            }
        }
        visit_mut::visit_expr_call_mut(self, call);
    }
}

#[derive(Default)]
struct WireDispatch {
    target: bool,
    owned: usize,
    views: usize,
    current_view: bool,
    seen_owned: bool,
    seen_views: bool,
    error: Option<String>,
}
impl VisitMut for WireDispatch {
    fn visit_item_impl_mut(&mut self, item: &mut syn::ItemImpl) {
        let (old, old_view) = (self.target, self.current_view);
        let name = if let syn::Type::Path(p) = item.self_ty.as_ref() {
            p.path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };
        self.target = matches!(
            name.as_str(),
            "ChatSolidColorWallpaper" | "ChatSolidColorWallpaperView"
        );
        self.current_view = name == "ChatSolidColorWallpaperView";
        let trait_name = item
            .trait_
            .as_ref()
            .and_then(|(path, _)| path.segments.last())
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        self.seen_owned |= name == "ChatSolidColorWallpaper" && trait_name == "Message";
        self.seen_views |= self.current_view && trait_name == "MessageView";
        visit_mut::visit_item_impl_mut(self, item);
        self.target = old;
        self.current_view = old_view;
    }
    fn visit_expr_match_mut(&mut self, node: &mut syn::ExprMatch) {
        if self.target
            && let syn::Expr::MethodCall(call) = node.expr.as_ref()
            && call.method == "field_number"
        {
            let mut count = 0;
            let mut alternatives = std::collections::BTreeSet::new();
            for arm in &mut node.arms {
                let syn::Pat::Lit(pattern) = &arm.pat else {
                    continue;
                };
                let syn::Lit::Int(number) = &pattern.lit else {
                    continue;
                };
                if number.suffix() != "u32" || !matches!(number.base10_digits(), "1" | "2") {
                    continue;
                }
                let mut checks = WireChecks::default();
                checks.visit_expr_mut(&mut arm.body);
                if checks.invalid || checks.values.len() != 1 {
                    self.error = Some("wallpaper decoder wire check changed".into());
                    continue;
                }
                let wire = &checks.values[0];
                let fixed = matches!(wire, syn::Expr::Path(path) if path.path.segments.last().is_some_and(|s|s.ident=="Fixed32"));
                if !alternatives.insert((number.base10_digits().to_owned(), fixed)) {
                    self.error = Some("duplicate wallpaper field/wire-type alternative".into());
                }
                let pattern = &arm.pat;
                arm.pat = syn::parse_quote!(#pattern if tag.wire_type()==#wire);
                count += 1;
            }
            if count != 4 {
                self.error = Some(format!(
                    "wallpaper decoder expected four disjoint arms, found {count}"
                ));
            }
            let fallback = node
                .arms
                .iter()
                .position(|arm| matches!(arm.pat, syn::Pat::Wild(_)));
            if let Some(index) = fallback {
                node.arms.insert(index,syn::parse_quote!(1u32|2u32 => {return Err(::buffa::encoding::wire_type_mismatch(tag,::buffa::encoding::WireType::Fixed32));}));
            } else {
                self.error = Some("wallpaper decoder lost unknown-field fallback".into());
            }
            if self.current_view {
                self.views += count;
            } else {
                self.owned += count;
            }
        }
        visit_mut::visit_expr_match_mut(self, node);
    }
}
pub(super) fn qualify(file: &mut syn::File) -> io::Result<()> {
    let mut visitor = WireDispatch::default();
    visitor.visit_file_mut(file);
    if let Some(error) = visitor.error {
        return Err(io::Error::other(error));
    }
    if visitor.owned != usize::from(visitor.seen_owned) * 4
        || visitor.views != usize::from(visitor.seen_views) * 4
    {
        return Err(io::Error::other(
            "wallpaper decoder emitted multiple wire dispatches",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_mentions_cannot_qualify_a_missing_wire_check() {
        let mut file: syn::File = syn::parse_quote! {
            impl Message for ChatSolidColorWallpaper {
                fn merge_field(&mut self, tag: Tag) {
                    match tag.field_number() {
                        1u32 => { let _ = "WireType :: Fixed32"; }
                        2u32 => { let _ = "WireType :: LengthDelimited"; }
                        _ => {}
                    }
                }
            }
        };
        assert!(qualify(&mut file).is_err());
    }
}
