use std::collections::{BTreeMap, BTreeSet};

use card_data_contract::{
    official::{CardsPageResponse, MetadataResponse, OfficialCard},
    raw::{CardListQuery, RawMetadata, RawPage, RawSnapshot, RawSource, RequestedCardResponse},
};
use serde::de::{DeserializeOwned, IntoDeserializer};
use serde_json::Value;

use crate::{
    clock::{utc_timestamp, Clock},
    BlizzardClient, PipelineError, RetryEvent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Scope {
    Related,
    ClassReference,
    Standard,
}

#[derive(Clone)]
pub struct CollectedLocale {
    pub locale: String,
    pub standard_cards: BTreeMap<i64, OfficialCard>,
    pub related_cards: BTreeMap<i64, OfficialCard>,
    pub class_reference_cards: BTreeMap<i64, OfficialCard>,
    pub metadata: MetadataResponse,
    pub raw: RawSnapshot,
}

#[derive(Clone)]
pub struct CollectedLocales {
    pub ko_kr: CollectedLocale,
    pub en_us: CollectedLocale,
}

pub struct Collector {
    client: BlizzardClient,
    clock: std::sync::Arc<dyn Clock>,
}

impl Collector {
    pub fn new(client: BlizzardClient) -> Self {
        let clock = client.clock();
        Self { client, clock }
    }

    pub async fn collect_all(&mut self) -> Result<CollectedLocales, PipelineError> {
        let ko_kr = self.collect_locale("ko_KR").await?;
        let en_us = self.collect_locale("en_US").await?;
        ensure_parity(&ko_kr, &en_us)?;
        Ok(CollectedLocales { ko_kr, en_us })
    }

    /// collection 중 발생한 secret-free retry event를 반환하고 buffer를 비운다.
    pub fn take_retry_events(&self) -> Vec<RetryEvent> {
        self.client.take_retry_events()
    }

    async fn collect_locale(&mut self, locale: &str) -> Result<CollectedLocale, PipelineError> {
        let collected_at = utc_timestamp(self.clock.as_ref()).map_err(|_| {
            PipelineError::ApiStructure("collection timestamp could not be formatted".into())
        })?;
        let first_value = self.client.fetch_cards_page_value(locale, 1).await?;
        let first = parse_page(first_value.clone())?;
        validate_page(&first, 1, first.page_count, first.card_count)?;
        if first.page_count < 1 {
            return Err(PipelineError::ApiStructure(
                "card list pageCount must be positive".into(),
            ));
        }
        let page_count = u32::try_from(first.page_count)
            .map_err(|_| PipelineError::ApiStructure("card list pageCount is invalid".into()))?;
        let expected_card_count = first.card_count;
        let mut pages = vec![(1, first_value, first)];
        for page in 2..=page_count {
            let value = self.client.fetch_cards_page_value(locale, page).await?;
            let parsed = parse_page(value.clone())?;
            validate_page(&parsed, page, i64::from(page_count), expected_card_count)?;
            pages.push((page, value, parsed));
        }
        let mut cards = BTreeMap::new();
        let mut scopes = BTreeMap::new();
        let mut raw_pages = Vec::new();
        for (page, value, parsed) in pages {
            raw_pages.push(RawPage {
                page,
                response: value,
            });
            for card in parsed.cards {
                let id = card.id;
                if cards.insert(id, card).is_some() {
                    return Err(PipelineError::ApiStructure(
                        "card list contains duplicate IDs".into(),
                    ));
                }
                scopes.insert(id, Scope::Standard);
            }
        }
        if i64::try_from(cards.len()).ok() != Some(expected_card_count) {
            return Err(PipelineError::ApiStructure(
                "card list total does not match cardCount".into(),
            ));
        }

        let metadata_value = self.client.fetch_metadata_value(locale).await?;
        let metadata = parse_metadata(metadata_value.clone())?;
        let excluded_related_targets = alternate_hero_targets(&metadata);
        let mut individual_responses = BTreeMap::new();
        let mut pending = forward_targets(&cards);
        pending.retain(|id| !excluded_related_targets.contains(id));
        while let Some(id) = pop_first(&mut pending) {
            if cards.contains_key(&id) {
                continue;
            }
            let value = self.client.fetch_card_value(locale, id).await?;
            let card = parse_card(value.clone())?;
            if card.id != id {
                return Err(PipelineError::ApiStructure(
                    "requested card ID does not match response ID".into(),
                ));
            }
            for target in card.child_ids.iter().chain(&card.bundled_card_ids) {
                if !cards.contains_key(target) && !excluded_related_targets.contains(target) {
                    pending.insert(*target);
                }
            }
            cards.insert(id, card);
            scopes.insert(id, Scope::Related);
            individual_responses.insert(id, value);
        }
        for id in class_targets(&metadata) {
            match scopes.get(&id).copied() {
                Some(Scope::Standard) | Some(Scope::ClassReference) => {}
                Some(Scope::Related) => {
                    scopes.insert(id, Scope::ClassReference);
                }
                None => {
                    let value = self.client.fetch_card_value(locale, id).await?;
                    let card = parse_card(value.clone())?;
                    if card.id != id {
                        return Err(PipelineError::ApiStructure(
                            "requested class card ID does not match response ID".into(),
                        ));
                    }
                    cards.insert(id, card);
                    scopes.insert(id, Scope::ClassReference);
                    individual_responses.insert(id, value);
                }
            }
        }
        let mut pending = forward_targets(&cards);
        pending.retain(|id| !excluded_related_targets.contains(id));
        while let Some(id) = pop_first(&mut pending) {
            if cards.contains_key(&id) {
                continue;
            }
            let value = self.client.fetch_card_value(locale, id).await?;
            let card = parse_card(value.clone())?;
            if card.id != id {
                return Err(PipelineError::ApiStructure(
                    "requested related card ID does not match response ID".into(),
                ));
            }
            for target in card.child_ids.iter().chain(&card.bundled_card_ids) {
                if !cards.contains_key(target) && !excluded_related_targets.contains(target) {
                    pending.insert(*target);
                }
            }
            cards.insert(id, card);
            scopes.insert(id, Scope::Related);
            individual_responses.insert(id, value);
        }
        let mut standard_cards = BTreeMap::new();
        let mut related_cards = BTreeMap::new();
        let mut class_reference_cards = BTreeMap::new();
        for (id, card) in cards {
            match scopes.get(&id).copied().expect("scope exists") {
                Scope::Standard => {
                    standard_cards.insert(id, card);
                }
                Scope::Related => {
                    related_cards.insert(id, card);
                }
                Scope::ClassReference => {
                    class_reference_cards.insert(id, card);
                }
            }
        }
        let mut related_raw = Vec::new();
        let mut class_raw = Vec::new();
        for (id, response) in individual_responses {
            let wrapper = RequestedCardResponse {
                requested_card_id: id,
                response,
            };
            match scopes.get(&id).copied().expect("scope exists") {
                Scope::Related => related_raw.push(wrapper),
                Scope::ClassReference => class_raw.push(wrapper),
                Scope::Standard => {}
            }
        }
        Ok(CollectedLocale {
            locale: locale.into(),
            standard_cards,
            related_cards,
            class_reference_cards,
            metadata,
            raw: RawSnapshot {
                format_version: 1,
                source: RawSource::blizzard_us(),
                collected_at,
                query: CardListQuery::standard(locale),
                card_pages: raw_pages,
                related_cards: related_raw,
                class_reference_cards: class_raw,
                metadata: RawMetadata {
                    response: metadata_value,
                },
            },
        })
    }
}

pub(crate) fn parse_page(value: Value) -> Result<CardsPageResponse, PipelineError> {
    parse_schema(value, "card list response")
}
pub(crate) fn parse_card(value: Value) -> Result<OfficialCard, PipelineError> {
    parse_schema(value, "card response")
}
pub(crate) fn parse_metadata(value: Value) -> Result<MetadataResponse, PipelineError> {
    let object = value
        .as_object()
        .ok_or_else(|| PipelineError::ApiStructure("metadata response must be an object".into()))?;
    for key in [
        "sets",
        "classes",
        "types",
        "rarities",
        "minionTypes",
        "spellSchools",
        "keywords",
    ] {
        if !object.get(key).is_some_and(Value::is_array) {
            return Err(PipelineError::ApiStructure(format!(
                "metadata response is missing required array {key}"
            )));
        }
    }
    parse_schema(value, "metadata response")
}

fn parse_schema<T: DeserializeOwned>(value: Value, response: &str) -> Result<T, PipelineError> {
    serde_path_to_error::deserialize(value.into_deserializer()).map_err(|error| {
        let mut path = error.path().to_string();
        if path.is_empty() {
            path = "<root>".into();
        }
        let category = missing_field(error.inner()).map_or("data", |field| {
            path.push('.');
            path.push_str(&field);
            "missing_field"
        });
        PipelineError::ApiStructure(format!("{response} schema error at {path}: {category}"))
    })
}

fn missing_field(error: &serde_json::Error) -> Option<String> {
    let message = error.to_string();
    let field = message.strip_prefix("missing field `")?.strip_suffix('`')?;
    field
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        .then(|| field.to_owned())
}
fn validate_page(
    page: &CardsPageResponse,
    expected_page: u32,
    page_count: i64,
    card_count: i64,
) -> Result<(), PipelineError> {
    if page.page != i64::from(expected_page)
        || page.page_count != page_count
        || page.card_count != card_count
    {
        return Err(PipelineError::ApiStructure(
            "card list page metadata drifted during collection".into(),
        ));
    }
    Ok(())
}
fn forward_targets(cards: &BTreeMap<i64, OfficialCard>) -> BTreeSet<i64> {
    cards
        .values()
        .flat_map(|card| card.child_ids.iter().chain(&card.bundled_card_ids))
        .copied()
        .collect()
}
fn class_targets(metadata: &MetadataResponse) -> BTreeSet<i64> {
    metadata
        .classes
        .iter()
        .flat_map(|class| [class.card_id, class.hero_power_card_id])
        .flatten()
        .collect()
}
fn alternate_hero_targets(metadata: &MetadataResponse) -> BTreeSet<i64> {
    metadata
        .classes
        .iter()
        .flat_map(|class| class.alternate_hero_card_ids.iter().copied())
        .collect()
}
fn pop_first(set: &mut BTreeSet<i64>) -> Option<i64> {
    let id = set.first().copied()?;
    set.remove(&id);
    Some(id)
}
fn ids(cards: &BTreeMap<i64, OfficialCard>) -> BTreeSet<i64> {
    cards.keys().copied().collect()
}
fn metadata_taxonomy_ids(locale: &CollectedLocale) -> Vec<BTreeSet<i64>> {
    vec![
        locale.metadata.sets.iter().map(|x| x.id).collect(),
        locale.metadata.classes.iter().map(|x| x.id).collect(),
        locale.metadata.types.iter().map(|x| x.id).collect(),
        locale.metadata.rarities.iter().map(|x| x.id).collect(),
        locale.metadata.minion_types.iter().map(|x| x.id).collect(),
        locale.metadata.spell_schools.iter().map(|x| x.id).collect(),
        locale.metadata.keywords.iter().map(|x| x.id).collect(),
    ]
}

fn card_taxonomy_reference_ids(locale: &CollectedLocale) -> Vec<BTreeSet<i64>> {
    let mut taxonomy = vec![BTreeSet::new(); 7];
    for card in locale
        .standard_cards
        .values()
        .chain(locale.related_cards.values())
        .chain(locale.class_reference_cards.values())
    {
        taxonomy[0].insert(card.card_set_id);
        if let Some(id) = card.class_id {
            taxonomy[1].insert(id);
        }
        taxonomy[1].extend(card.multi_class_ids.iter().copied());
        taxonomy[2].insert(card.card_type_id);
        if let Some(id) = card.rarity_id {
            taxonomy[3].insert(id);
        }
        if let Some(id) = card.minion_type_id {
            taxonomy[4].insert(id);
        }
        taxonomy[4].extend(card.multi_type_ids.iter().copied());
        if let Some(id) = card.spell_school_id {
            taxonomy[5].insert(id);
        }
        taxonomy[6].extend(card.keyword_ids.iter().copied());
    }
    taxonomy
}
fn ensure_parity(ko: &CollectedLocale, en: &CollectedLocale) -> Result<(), PipelineError> {
    if ids(&ko.standard_cards) != ids(&en.standard_cards)
        || ids(&ko.related_cards) != ids(&en.related_cards)
        || ids(&ko.class_reference_cards) != ids(&en.class_reference_cards)
        || metadata_taxonomy_ids(ko) != metadata_taxonomy_ids(en)
        || card_taxonomy_reference_ids(ko) != card_taxonomy_reference_ids(en)
    {
        return Err(PipelineError::ApiStructure(
            "ko_KR and en_US collection structures differ".into(),
        ));
    }
    Ok(())
}
