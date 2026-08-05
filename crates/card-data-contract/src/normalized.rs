use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NormalizedContractError {
    #[error("unsupported locale: {0}")]
    UnsupportedLocale(String),
    #[error("source Raw SHA-256 must be 64 lowercase hexadecimal characters")]
    InvalidRawHash,
    #[error("total card count must equal the sum of scope counts")]
    InvalidCardCounts,
    #[error("relation kind {kind:?} must use source field {expected:?}, not {actual:?}")]
    InvalidRelationSource {
        kind: RelationKind,
        expected: SourceField,
        actual: SourceField,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedCatalog {
    pub locale: String,
    pub generated_at: String,
    pub source_raw_sha256: String,
    pub card_counts: CardCounts,
    pub sets: Vec<TaxonomyRow>,
    pub classes: Vec<ClassRow>,
    pub card_types: Vec<TaxonomyRow>,
    pub rarities: Vec<RarityRow>,
    pub minion_types: Vec<TaxonomyRow>,
    pub spell_schools: Vec<TaxonomyRow>,
    pub keywords: Vec<KeywordRow>,
    pub cards: Vec<NormalizedCard>,
    pub card_classes: Vec<CardTaxonomyJoin>,
    pub card_minion_types: Vec<CardTaxonomyJoin>,
    pub card_keywords: Vec<CardKeywordJoin>,
    pub relations: Vec<CardRelation>,
}

impl NormalizedCatalog {
    pub fn validate(&self) -> Result<(), NormalizedContractError> {
        if !matches!(self.locale.as_str(), "ko_KR" | "en_US") {
            return Err(NormalizedContractError::UnsupportedLocale(
                self.locale.clone(),
            ));
        }
        if !is_lowercase_sha256(&self.source_raw_sha256) {
            return Err(NormalizedContractError::InvalidRawHash);
        }
        if !self.card_counts.is_valid() {
            return Err(NormalizedContractError::InvalidCardCounts);
        }
        for relation in &self.relations {
            relation.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Standard,
    ClassReference,
    Related,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardCounts {
    pub standard: u64,
    pub related: u64,
    pub class_reference: u64,
    pub total: u64,
}

impl CardCounts {
    pub fn is_valid(self) -> bool {
        self.total
            == self
                .standard
                .saturating_add(self.related)
                .saturating_add(self.class_reference)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxonomyRow {
    pub id: i64,
    pub slug: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassRow {
    pub id: i64,
    pub slug: Option<String>,
    pub name: Option<String>,
    pub default_hero_card_id: Option<i64>,
    pub default_hero_power_card_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RarityRow {
    pub id: i64,
    pub slug: Option<String>,
    pub name: Option<String>,
    pub crafting_cost_normal: Option<i64>,
    pub crafting_cost_golden: Option<i64>,
    pub dust_value_normal: Option<i64>,
    pub dust_value_golden: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeywordRow {
    pub id: i64,
    pub slug: Option<String>,
    pub name: Option<String>,
    pub ref_text: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedCard {
    pub id: i64,
    pub slug: String,
    pub scope_kind: ScopeKind,
    pub collectible: bool,
    pub name: Option<String>,
    pub text_markup: Option<String>,
    pub text_plain: Option<String>,
    pub flavor_text: Option<String>,
    pub artist_name: Option<String>,
    pub mana_cost: i64,
    pub attack: Option<i64>,
    pub health: Option<i64>,
    pub armor: Option<i64>,
    pub deck_size_mod: Option<i64>,
    pub set_id: i64,
    pub type_id: i64,
    pub rarity_id: Option<i64>,
    pub spell_school_id: Option<i64>,
    pub image_url: Option<String>,
    pub crop_image_url: Option<String>,
    pub rune_blood: Option<i64>,
    pub rune_frost: Option<i64>,
    pub rune_unholy: Option<i64>,
    pub sideboard_max_cards: Option<i64>,
    pub sideboard_subset: Option<String>,
    pub sideboard_ignores_class: Option<bool>,
    pub sideboard_cards_count_as_max: Option<bool>,
    pub banned_from_sideboard: bool,
    pub zilliax_functional_module: bool,
    pub zilliax_cosmetic_module: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinSourceKind {
    Primary,
    Multi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardTaxonomyJoin {
    pub card_id: i64,
    pub taxonomy_id: i64,
    pub position: u32,
    pub source_kind: JoinSourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardKeywordJoin {
    pub card_id: i64,
    pub keyword_id: i64,
    pub position: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationKind {
    #[serde(rename = "child")]
    Child,
    #[serde(rename = "bundled")]
    Bundled,
    #[serde(rename = "parent")]
    Parent,
    #[serde(rename = "copy_of")]
    CopyOf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceField {
    #[serde(rename = "childIds")]
    ChildIds,
    #[serde(rename = "bundledCardIds")]
    BundledCardIds,
    #[serde(rename = "parentId")]
    ParentId,
    #[serde(rename = "copyOfCardId")]
    CopyOfCardId,
}

impl RelationKind {
    pub const fn source_field(self) -> SourceField {
        match self {
            Self::Child => SourceField::ChildIds,
            Self::Bundled => SourceField::BundledCardIds,
            Self::Parent => SourceField::ParentId,
            Self::CopyOf => SourceField::CopyOfCardId,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardRelation {
    pub source_card_id: i64,
    pub relation_kind: RelationKind,
    pub source_field: SourceField,
    pub target_card_id: i64,
    pub display_order: u32,
}

impl CardRelation {
    pub fn validate(&self) -> Result<(), NormalizedContractError> {
        let expected = self.relation_kind.source_field();
        if self.source_field != expected {
            return Err(NormalizedContractError::InvalidRelationSource {
                kind: self.relation_kind,
                expected,
                actual: self.source_field,
            });
        }
        Ok(())
    }
}

pub(crate) fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::{CardRelation, RelationKind, SourceField};

    #[test]
    fn relation_rejects_a_mismatched_official_source_field() {
        let relation = CardRelation {
            source_card_id: 1,
            relation_kind: RelationKind::Child,
            source_field: SourceField::CopyOfCardId,
            target_card_id: 2,
            display_order: 0,
        };

        assert!(relation.validate().is_err());
    }
}
