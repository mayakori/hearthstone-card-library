use std::collections::{BTreeMap, BTreeSet};

use card_data_contract::normalized::{
    CardKeywordJoin, CardTaxonomyJoin, ClassRow, JoinSourceKind, KeywordRow, NormalizedCard,
    RarityRow, ScopeKind, TaxonomyRow,
};
use card_data_contract::{
    plain_text, CardCounts, CardRelation, NormalizedCatalog, RelationKind, SourceField,
};

use crate::{CollectedLocale, PipelineError};

pub fn normalize_locale(
    locale: &CollectedLocale,
    source_raw_sha256: &str,
    generated_at: &str,
) -> Result<NormalizedCatalog, PipelineError> {
    if !matches!(locale.locale.as_str(), "ko_KR" | "en_US") {
        return Err(normalize_error("unsupported locale"));
    }
    let mut cards = BTreeMap::new();
    for (scope, rows) in [
        (ScopeKind::Related, &locale.related_cards),
        (ScopeKind::ClassReference, &locale.class_reference_cards),
        (ScopeKind::Standard, &locale.standard_cards),
    ] {
        for (&id, card) in rows {
            if card.id != id {
                return Err(normalize_error(
                    "card map key does not match official card ID",
                ));
            }
            cards.insert(id, (scope, card));
        }
    }
    let counts = CardCounts {
        standard: cards
            .values()
            .filter(|(scope, _)| *scope == ScopeKind::Standard)
            .count() as u64,
        related: cards
            .values()
            .filter(|(scope, _)| *scope == ScopeKind::Related)
            .count() as u64,
        class_reference: cards
            .values()
            .filter(|(scope, _)| *scope == ScopeKind::ClassReference)
            .count() as u64,
        total: cards.len() as u64,
    };
    let mut catalog = NormalizedCatalog {
        locale: locale.locale.clone(),
        generated_at: generated_at.into(),
        source_raw_sha256: source_raw_sha256.into(),
        card_counts: counts,
        sets: taxonomy(&locale.metadata.sets)?,
        classes: classes(locale)?,
        card_types: taxonomy(&locale.metadata.types)?,
        rarities: rarities(locale)?,
        minion_types: taxonomy(&locale.metadata.minion_types)?,
        spell_schools: taxonomy(&locale.metadata.spell_schools)?,
        keywords: keywords(locale)?,
        cards: Vec::new(),
        card_classes: Vec::new(),
        card_minion_types: Vec::new(),
        card_keywords: Vec::new(),
        relations: Vec::new(),
    };
    for (&id, (scope, card)) in &cards {
        if card.collectible != 0 && card.collectible != 1 {
            return Err(normalize_error("collectible must be zero or one"));
        }
        non_negative("manaCost", card.mana_cost)?;
        ensure_taxonomy(&mut catalog.sets, card.card_set_id);
        ensure_taxonomy(&mut catalog.card_types, card.card_type_id);
        if let Some(value) = card.rarity_id {
            ensure_rarity(&mut catalog.rarities, value);
        }
        if let Some(value) = card.spell_school_id {
            ensure_taxonomy(&mut catalog.spell_schools, value);
        }
        let rune = match &card.rune_cost {
            Some(value) => {
                non_negative("runeCost.blood", value.blood)?;
                non_negative("runeCost.frost", value.frost)?;
                non_negative("runeCost.unholy", value.unholy)?;
                (Some(value.blood), Some(value.frost), Some(value.unholy))
            }
            None => (None, None, None),
        };
        let sideboard = match &card.sideboard {
            Some(value) => {
                non_negative("sideboard.maxCards", value.max_cards)?;
                (
                    Some(value.max_cards),
                    Some(value.sideboard_subset.clone()),
                    Some(value.ignores_class),
                    Some(value.cards_count_as_max),
                )
            }
            None => (None, None, None, None),
        };
        let markup = empty_to_none(card.text.as_deref());
        catalog.cards.push(NormalizedCard {
            id,
            slug: card.slug.clone(),
            scope_kind: *scope,
            collectible: card.collectible == 1,
            name: empty_to_none(card.name.as_deref()),
            text_markup: markup.clone(),
            text_plain: markup.as_deref().map(plain_text),
            flavor_text: empty_to_none(card.flavor_text.as_deref()),
            artist_name: empty_to_none(card.artist_name.as_deref()),
            mana_cost: card.mana_cost,
            attack: card.attack,
            health: card.health,
            armor: card.armor,
            deck_size_mod: card.deck_size,
            set_id: card.card_set_id,
            type_id: card.card_type_id,
            rarity_id: card.rarity_id,
            spell_school_id: card.spell_school_id,
            image_url: empty_to_none(card.image.as_deref()),
            crop_image_url: empty_to_none(card.crop_image.as_deref()),
            rune_blood: rune.0,
            rune_frost: rune.1,
            rune_unholy: rune.2,
            sideboard_max_cards: sideboard.0,
            sideboard_subset: sideboard.1,
            sideboard_ignores_class: sideboard.2,
            sideboard_cards_count_as_max: sideboard.3,
            banned_from_sideboard: card.banned_from_sideboard.unwrap_or(false),
            zilliax_functional_module: card.is_zilliax_functional_module.unwrap_or(false),
            zilliax_cosmetic_module: card.is_zilliax_cosmetic_module.unwrap_or(false),
        });
        add_taxonomy_joins(&mut catalog, id, card.class_id, &card.multi_class_ids, true)?;
        add_taxonomy_joins(
            &mut catalog,
            id,
            card.minion_type_id,
            &card.multi_type_ids,
            false,
        )?;
        add_keywords(&mut catalog, id, &card.keyword_ids)?;
        add_relations(&mut catalog, id, card)?;
    }
    catalog.sets.sort_by_key(|row| row.id);
    catalog.classes.sort_by_key(|row| row.id);
    catalog.card_types.sort_by_key(|row| row.id);
    catalog.rarities.sort_by_key(|row| row.id);
    catalog.minion_types.sort_by_key(|row| row.id);
    catalog.spell_schools.sort_by_key(|row| row.id);
    catalog.keywords.sort_by_key(|row| row.id);
    catalog.cards.sort_by_key(|row| row.id);
    catalog
        .card_classes
        .sort_by_key(|row| (row.card_id, row.position));
    catalog
        .card_minion_types
        .sort_by_key(|row| (row.card_id, row.position));
    catalog
        .card_keywords
        .sort_by_key(|row| (row.card_id, row.position));
    catalog.relations.sort_by_key(|row| {
        (
            row.source_card_id,
            relation_kind_name(row.relation_kind),
            row.display_order,
        )
    });
    let card_ids = catalog
        .cards
        .iter()
        .map(|card| card.id)
        .collect::<BTreeSet<_>>();
    let card_scopes = catalog
        .cards
        .iter()
        .map(|card| (card.id, card.scope_kind))
        .collect::<BTreeMap<_, _>>();
    let alternate_hero_ids = locale
        .metadata
        .classes
        .iter()
        .flat_map(|class| class.alternate_hero_card_ids.iter().copied())
        .collect::<BTreeSet<_>>();
    if catalog.relations.iter().any(|relation| {
        matches!(
            relation.relation_kind,
            RelationKind::Child | RelationKind::Bundled
        ) && !card_ids.contains(&relation.target_card_id)
            && !(card_scopes.get(&relation.source_card_id) == Some(&ScopeKind::ClassReference)
                && alternate_hero_ids.contains(&relation.target_card_id))
    }) {
        return Err(normalize_error(
            "non-skin childIds and bundledCardIds targets must be collected cards",
        ));
    }
    catalog
        .validate()
        .map_err(|error| normalize_error(error.to_string()))?;
    Ok(catalog)
}

fn normalize_error(message: impl Into<String>) -> PipelineError {
    PipelineError::Normalize(message.into())
}
fn non_negative(name: &str, value: i64) -> Result<(), PipelineError> {
    if value < 0 {
        Err(normalize_error(format!("{name} must not be negative")))
    } else {
        Ok(())
    }
}
fn currency_pair(
    values: Option<[Option<i64>; 2]>,
    name: &str,
) -> Result<(Option<i64>, Option<i64>), PipelineError> {
    let Some([normal, golden]) = values else {
        return Ok((None, None));
    };
    for amount in [normal, golden].into_iter().flatten() {
        non_negative(name, amount)?;
    }
    Ok((normal, golden))
}
fn empty_to_none(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.is_empty()).map(str::to_owned)
}
fn taxonomy(
    values: &[card_data_contract::official::MetadataEntry],
) -> Result<Vec<TaxonomyRow>, PipelineError> {
    let mut ids = BTreeSet::new();
    values
        .iter()
        .map(|value| {
            if !ids.insert(value.id) {
                return Err(normalize_error("metadata taxonomy has duplicate ID"));
            }
            Ok(TaxonomyRow {
                id: value.id,
                slug: empty_to_none(value.slug.as_deref()),
                name: empty_to_none(value.name.as_deref()),
            })
        })
        .collect()
}
fn classes(locale: &CollectedLocale) -> Result<Vec<ClassRow>, PipelineError> {
    let mut ids = BTreeSet::new();
    locale
        .metadata
        .classes
        .iter()
        .map(|value| {
            if !ids.insert(value.id) {
                return Err(normalize_error("metadata class has duplicate ID"));
            }
            Ok(ClassRow {
                id: value.id,
                slug: empty_to_none(value.slug.as_deref()),
                name: empty_to_none(value.name.as_deref()),
                default_hero_card_id: value.card_id,
                default_hero_power_card_id: value.hero_power_card_id,
            })
        })
        .collect()
}
fn rarities(locale: &CollectedLocale) -> Result<Vec<RarityRow>, PipelineError> {
    let mut ids = BTreeSet::new();
    locale
        .metadata
        .rarities
        .iter()
        .map(|value| {
            if !ids.insert(value.id) {
                return Err(normalize_error("metadata rarity has duplicate ID"));
            }
            let crafting_cost = currency_pair(value.crafting_cost, "craftingCost")?;
            let dust_value = currency_pair(value.dust_value, "dustValue")?;
            Ok(RarityRow {
                id: value.id,
                slug: empty_to_none(value.slug.as_deref()),
                name: empty_to_none(value.name.as_deref()),
                crafting_cost_normal: crafting_cost.0,
                crafting_cost_golden: crafting_cost.1,
                dust_value_normal: dust_value.0,
                dust_value_golden: dust_value.1,
            })
        })
        .collect()
}
fn keywords(locale: &CollectedLocale) -> Result<Vec<KeywordRow>, PipelineError> {
    let mut ids = BTreeSet::new();
    locale
        .metadata
        .keywords
        .iter()
        .map(|value| {
            if !ids.insert(value.id) {
                return Err(normalize_error("metadata keyword has duplicate ID"));
            }
            Ok(KeywordRow {
                id: value.id,
                slug: empty_to_none(value.slug.as_deref()),
                name: empty_to_none(value.name.as_deref()),
                ref_text: empty_to_none(value.ref_text.as_deref()),
                text: empty_to_none(value.text.as_deref()),
            })
        })
        .collect()
}
fn ensure_taxonomy(rows: &mut Vec<TaxonomyRow>, id: i64) {
    if !rows.iter().any(|row| row.id == id) {
        rows.push(TaxonomyRow {
            id,
            slug: None,
            name: None,
        });
    }
}
fn ensure_rarity(rows: &mut Vec<RarityRow>, id: i64) {
    if !rows.iter().any(|row| row.id == id) {
        rows.push(RarityRow {
            id,
            slug: None,
            name: None,
            crafting_cost_normal: None,
            crafting_cost_golden: None,
            dust_value_normal: None,
            dust_value_golden: None,
        });
    }
}
fn ensure_class(rows: &mut Vec<ClassRow>, id: i64) {
    if !rows.iter().any(|row| row.id == id) {
        rows.push(ClassRow {
            id,
            slug: None,
            name: None,
            default_hero_card_id: None,
            default_hero_power_card_id: None,
        });
    }
}
fn ensure_keyword(rows: &mut Vec<KeywordRow>, id: i64) {
    if !rows.iter().any(|row| row.id == id) {
        rows.push(KeywordRow {
            id,
            slug: None,
            name: None,
            ref_text: None,
            text: None,
        });
    }
}
fn add_taxonomy_joins(
    catalog: &mut NormalizedCatalog,
    card_id: i64,
    primary: Option<i64>,
    multi: &[i64],
    is_class: bool,
) -> Result<(), PipelineError> {
    let mut ids = BTreeSet::new();
    let mut multi_ids = BTreeSet::new();
    let mut position = 0;
    if let Some(id) = primary {
        ids.insert(id);
        if is_class {
            ensure_class(&mut catalog.classes, id);
        } else {
            ensure_taxonomy(&mut catalog.minion_types, id);
        }
        let join = CardTaxonomyJoin {
            card_id,
            taxonomy_id: id,
            position,
            source_kind: JoinSourceKind::Primary,
        };
        if is_class {
            catalog.card_classes.push(join);
        } else {
            catalog.card_minion_types.push(join);
        }
        position += 1;
    }
    for &id in multi {
        if !multi_ids.insert(id) {
            return Err(normalize_error("official taxonomy array has duplicate ID"));
        }
        // `multiTypeIds`/`multiClassIds` is an additional array. Some official
        // cards repeat their primary value there; the table's uniqueness contract
        // retains the primary row and does not manufacture a duplicate join.
        if !ids.insert(id) {
            continue;
        }
        if is_class {
            ensure_class(&mut catalog.classes, id);
        } else {
            ensure_taxonomy(&mut catalog.minion_types, id);
        }
        let join = CardTaxonomyJoin {
            card_id,
            taxonomy_id: id,
            position,
            source_kind: JoinSourceKind::Multi,
        };
        if is_class {
            catalog.card_classes.push(join);
        } else {
            catalog.card_minion_types.push(join);
        }
        position += 1;
    }
    Ok(())
}
fn add_keywords(
    catalog: &mut NormalizedCatalog,
    card_id: i64,
    keywords: &[i64],
) -> Result<(), PipelineError> {
    let mut ids = BTreeSet::new();
    for (position, &keyword_id) in keywords.iter().enumerate() {
        if !ids.insert(keyword_id) {
            return Err(normalize_error("keywordIds has duplicate ID"));
        }
        ensure_keyword(&mut catalog.keywords, keyword_id);
        catalog.card_keywords.push(CardKeywordJoin {
            card_id,
            keyword_id,
            position: position as u32,
        });
    }
    Ok(())
}
fn add_relations(
    catalog: &mut NormalizedCatalog,
    card_id: i64,
    card: &card_data_contract::OfficialCard,
) -> Result<(), PipelineError> {
    let groups: [(RelationKind, SourceField, Vec<i64>); 4] = [
        (
            RelationKind::Child,
            SourceField::ChildIds,
            card.child_ids.clone(),
        ),
        (
            RelationKind::Bundled,
            SourceField::BundledCardIds,
            card.bundled_card_ids.clone(),
        ),
        (
            RelationKind::Parent,
            SourceField::ParentId,
            card.parent_id.into_iter().collect(),
        ),
        (
            RelationKind::CopyOf,
            SourceField::CopyOfCardId,
            card.copy_of_card_id.clone(),
        ),
    ];
    for (kind, source_field, ids) in groups {
        let mut unique = BTreeSet::new();
        for (display_order, target_card_id) in ids.into_iter().enumerate() {
            if !unique.insert(target_card_id) {
                return Err(normalize_error("official relation array has duplicate ID"));
            }
            catalog.relations.push(CardRelation {
                source_card_id: card_id,
                relation_kind: kind,
                source_field,
                target_card_id,
                display_order: display_order as u32,
            });
        }
    }
    Ok(())
}
fn relation_kind_name(kind: RelationKind) -> &'static str {
    match kind {
        RelationKind::Child => "child",
        RelationKind::Bundled => "bundled",
        RelationKind::Parent => "parent",
        RelationKind::CopyOf => "copy_of",
    }
}
