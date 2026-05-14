//! Fluent API для привязки морфологических отношений к правилам.
//!
//! Расширяет [`RuleBuilder`] трейтом [`RuleBuilderRelationExt`] и методом
//! [`match_relation`](RuleBuilderRelationExt::match_relation), который оборачивает правило
//! в [`RuleKind::Relation`] с выбранным отношением.

use std::sync::Arc;

use crate::relations::graph::Relation;
use crate::rule::builder::RuleBuilder;
use crate::rule::constructors::{Rule, RuleKind};

/// Расширение [`RuleBuilder`] для привязки морфологических отношений.
pub trait RuleBuilderRelationExt<'a>: Sized {
    /// Оборачивает правило в [`RuleKind::Relation`] с заданным отношением.
    ///
    /// Токены, совпавшие с этим правилом и с другими правилами, привязанными
    /// к тому же экземпляру `relation` (`Arc`), будут проверены на морфологическую
    /// совместимость во время `prepare_match`.
    fn match_relation(self, relation: Arc<dyn Relation>) -> Self;
}

impl<'a> RuleBuilderRelationExt<'a> for RuleBuilder<'a> {
    fn match_relation(self, relation: Arc<dyn Relation>) -> Self {
        RuleBuilder::from_arc(Arc::new(Rule {
            kind: RuleKind::Relation {
                rule: self.into_arc(),
                relation,
            },
        }))
    }
}
