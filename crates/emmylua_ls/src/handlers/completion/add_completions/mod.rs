mod add_decl_completion;
mod add_member_completion;

pub use add_decl_completion::add_decl_completion;
pub use add_member_completion::{add_member_completion, CompletionTriggerStatus};
use code_analysis::{LuaPropertyOwnerId, LuaType};
use lsp_types::CompletionItemKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::util::humanize_type;

use super::completion_builder::CompletionBuilder;

fn check_visibility(builder: &CompletionBuilder, id: LuaPropertyOwnerId) -> Option<()> {
    match id {
        LuaPropertyOwnerId::Member(_) => {}
        LuaPropertyOwnerId::LuaDecl(_) => {}
        _ => return Some(()),
    }

    if !builder
        .semantic_model
        .is_property_visiable(builder.trigger_token.clone(), id)
    {
        return None;
    }

    Some(())
}

fn get_completion_kind(typ: &LuaType) -> CompletionItemKind {
    if typ.is_function() {
        return CompletionItemKind::FUNCTION;
    } else if typ.is_const() {
        return CompletionItemKind::CONSTANT;
    } else if typ.is_def() {
        return CompletionItemKind::CLASS;
    }

    CompletionItemKind::VARIABLE
}

fn is_deprecated(builder: &CompletionBuilder, id: LuaPropertyOwnerId) -> bool {
    let property = builder
        .semantic_model
        .get_db()
        .get_property_index()
        .get_property(id);
    if property.is_none() {
        return false;
    }

    property.unwrap().is_deprecated
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CallDisplay {
    None,
    AddSelf,
    RemoveFirst,
}

fn get_detail(
    builder: &CompletionBuilder,
    property_owner_id: &LuaPropertyOwnerId,
    typ: &LuaType,
    display: CallDisplay,
) -> Option<String> {
    match typ {
        LuaType::Signature(signature_id) => {
            let signature = builder
                .semantic_model
                .get_db()
                .get_signature_index()
                .get(&signature_id)?;

            let mut params_str = signature
                .get_type_params()
                .iter()
                .map(|param| param.0.clone())
                .collect::<Vec<_>>();

            match display {
                CallDisplay::AddSelf => {
                    params_str.insert(0, "self".to_string());
                }
                CallDisplay::RemoveFirst => {
                    if !params_str.is_empty() {
                        params_str.remove(0);
                    }
                }
                _ => {}
            }

            Some(format!("({})", params_str.join(", ")))
        }
        LuaType::DocFunction(f) => {
            let mut params_str = f
                .get_params()
                .iter()
                .map(|param| param.0.clone())
                .collect::<Vec<_>>();

            match display {
                CallDisplay::AddSelf => {
                    params_str.insert(0, "self".to_string());
                }
                CallDisplay::RemoveFirst => {
                    if !params_str.is_empty() {
                        params_str.remove(0);
                    }
                }
                _ => {}
            }

            Some(format!("({})", params_str.join(", ")))
        }
        _ => {
            // show comment in detail
            let property = builder
                .semantic_model
                .get_db()
                .get_property_index()
                .get_property(property_owner_id.clone())?;

            if let Some(detail) = &property.description {
                Some(truncate_with_ellipsis(detail, 25))
            } else {
                None
            }
        }
    }
}

fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len).collect();
        format!("   {}...", truncated)
    } else {
        format!("   {}", s)
    }
}

fn get_description(builder: &CompletionBuilder, typ: &LuaType) -> Option<String> {
    match typ {
        LuaType::Signature(signature_id) => {
            let signature = builder
                .semantic_model
                .get_db()
                .get_signature_index()
                .get(&signature_id)?;
            let rets = &signature.return_docs;
            if rets.len() == 1 {
                let detail = humanize_type(builder.semantic_model.get_db(), &rets[0].type_ref);
                Some(detail)
            } else if rets.len() > 1 {
                let detail = humanize_type(builder.semantic_model.get_db(), &rets[0].type_ref);
                Some(format!("{} ...", detail))
            } else {
                None
            }
        }
        LuaType::DocFunction(f) => {
            let rets = f.get_ret();
            if rets.len() == 1 {
                let detail = humanize_type(builder.semantic_model.get_db(), &rets[0]);
                Some(detail)
            } else if rets.len() > 1 {
                let detail = humanize_type(builder.semantic_model.get_db(), &rets[0]);
                Some(format!("{} ...", detail))
            } else {
                None
            }
        }
        _ if typ.is_unknown() => None,
        _ => Some(humanize_type(builder.semantic_model.get_db(), typ)),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompletionData {
    PropertyOwnerId(LuaPropertyOwnerId),
    Module(String),
}

#[allow(unused)]
impl CompletionData {
    pub fn from_property_owner_id(id: LuaPropertyOwnerId) -> Option<Value> {
        let data = Self::PropertyOwnerId(id);
        Some(serde_json::to_value(data).unwrap())
    }

    pub fn from_module(module: String) -> Option<Value> {
        let data = Self::Module(module);
        Some(serde_json::to_value(data).unwrap())
    }
}
