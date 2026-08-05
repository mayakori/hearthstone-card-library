use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const CARDS_ENDPOINT: &str = "https://us.api.blizzard.com/hearthstone/cards";
const CARD_BY_ID_TEMPLATE: &str = "https://us.api.blizzard.com/hearthstone/cards/{card-id}";
const METADATA_ENDPOINT: &str = "https://us.api.blizzard.com/hearthstone/metadata";

#[derive(Debug, Error)]
pub enum RawContractError {
    #[error("JSON serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawSnapshot {
    pub format_version: u32,
    pub source: RawSource,
    pub collected_at: String,
    pub query: CardListQuery,
    pub card_pages: Vec<RawPage>,
    pub related_cards: Vec<RequestedCardResponse>,
    pub class_reference_cards: Vec<RequestedCardResponse>,
    pub metadata: RawMetadata,
}

impl RawSnapshot {
    pub fn validate(&self) -> Result<(), RawContractError> {
        if self.format_version != 1 {
            return Err(RawContractError::Invalid(
                "Raw format_version must be 1".into(),
            ));
        }
        self.source.validate()?;
        self.query.validate()?;
        validate_ascending_pages(&self.card_pages)?;
        validate_ascending_requests("related_cards", &self.related_cards)?;
        validate_ascending_requests("class_reference_cards", &self.class_reference_cards)?;
        for response in self
            .card_pages
            .iter()
            .map(|page| &page.response)
            .chain(self.related_cards.iter().map(|card| &card.response))
            .chain(self.class_reference_cards.iter().map(|card| &card.response))
            .chain(std::iter::once(&self.metadata.response))
        {
            reject_secret_shaped_fields(response)?;
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RawContractError> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.push(b'{');
        write_named_json(&mut bytes, "format_version", &self.format_version)?;
        bytes.push(b',');
        write_named_json(&mut bytes, "source", &self.source)?;
        bytes.push(b',');
        write_named_json(&mut bytes, "collected_at", &self.collected_at)?;
        bytes.push(b',');
        write_named_json(&mut bytes, "query", &self.query)?;
        bytes.push(b',');
        write_pages(&mut bytes, &self.card_pages)?;
        bytes.push(b',');
        write_requested_cards(&mut bytes, "related_cards", &self.related_cards)?;
        bytes.push(b',');
        write_requested_cards(
            &mut bytes,
            "class_reference_cards",
            &self.class_reference_cards,
        )?;
        bytes.push(b',');
        bytes.extend(serde_json::to_vec("metadata")?);
        bytes.extend(b":{\"response\":");
        write_canonical_json_without_lf(&mut bytes, &self.metadata.response)?;
        bytes.extend(b"}}");
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSource {
    pub provider: String,
    pub api: String,
    pub region: String,
    pub endpoints: RawEndpoints,
}

impl RawSource {
    pub fn blizzard_us() -> Self {
        Self {
            provider: "blizzard".into(),
            api: "hearthstone_game_data".into(),
            region: "us".into(),
            endpoints: RawEndpoints::fixed(),
        }
    }

    pub fn validate(&self) -> Result<(), RawContractError> {
        if self.provider != "blizzard"
            || self.api != "hearthstone_game_data"
            || self.region != "us"
            || self.endpoints != RawEndpoints::fixed()
        {
            return Err(RawContractError::Invalid(
                "Raw source must use the fixed Blizzard US provenance".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawEndpoints {
    pub cards: String,
    pub card_by_id_template: String,
    pub metadata: String,
}

impl RawEndpoints {
    pub fn fixed() -> Self {
        Self {
            cards: CARDS_ENDPOINT.into(),
            card_by_id_template: CARD_BY_ID_TEMPLATE.into(),
            metadata: METADATA_ENDPOINT.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardListQuery {
    pub locale: String,
    #[serde(rename = "set")]
    pub set_name: String,
    #[serde(rename = "gameMode")]
    pub game_mode: String,
    pub collectible: String,
    #[serde(rename = "pageSize")]
    pub page_size: u32,
}

impl CardListQuery {
    pub fn standard(locale: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
            set_name: "standard".into(),
            game_mode: "constructed".into(),
            collectible: "0,1".into(),
            page_size: 500,
        }
    }

    pub fn validate(&self) -> Result<(), RawContractError> {
        if !matches!(self.locale.as_str(), "ko_KR" | "en_US")
            || self.set_name != "standard"
            || self.game_mode != "constructed"
            || self.collectible != "0,1"
            || self.page_size != 500
        {
            return Err(RawContractError::Invalid(
                "Raw query must be the fixed Standard card query".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawPage {
    pub page: u32,
    pub response: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestedCardResponse {
    pub requested_card_id: i64,
    pub response: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawMetadata {
    pub response: Value,
}

pub fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, RawContractError> {
    let canonical = canonicalize(value);
    let mut bytes = serde_json::to_vec(&canonical)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonicalize).collect()),
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), canonicalize(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn validate_ascending_pages(pages: &[RawPage]) -> Result<(), RawContractError> {
    if pages
        .iter()
        .map(|page| page.page)
        .collect::<Vec<_>>()
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(RawContractError::Invalid(
            "card_pages must be in ascending page order".into(),
        ));
    }
    Ok(())
}

fn validate_ascending_requests(
    field: &str,
    responses: &[RequestedCardResponse],
) -> Result<(), RawContractError> {
    if responses
        .iter()
        .map(|response| response.requested_card_id)
        .collect::<Vec<_>>()
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(RawContractError::Invalid(format!(
            "{field} must be in ascending requested_card_id order"
        )));
    }
    Ok(())
}

fn reject_secret_shaped_fields(value: &Value) -> Result<(), RawContractError> {
    const FORBIDDEN: [&str; 6] = [
        "clientid",
        "clientsecret",
        "accesstoken",
        "refreshtoken",
        "authorization",
        "oauth",
    ];
    match value {
        Value::Array(values) => {
            for value in values {
                reject_secret_shaped_fields(value)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                let normalized = key
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                if FORBIDDEN
                    .iter()
                    .any(|forbidden| normalized.contains(forbidden))
                {
                    return Err(RawContractError::Invalid(format!(
                        "Raw response contains forbidden secret-shaped field: {key}"
                    )));
                }
                reject_secret_shaped_fields(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn write_named_json<T: Serialize>(
    bytes: &mut Vec<u8>,
    name: &str,
    value: &T,
) -> Result<(), RawContractError> {
    bytes.extend(serde_json::to_vec(name)?);
    bytes.push(b':');
    bytes.extend(serde_json::to_vec(value)?);
    Ok(())
}

fn write_pages(bytes: &mut Vec<u8>, pages: &[RawPage]) -> Result<(), RawContractError> {
    bytes.extend(serde_json::to_vec("card_pages")?);
    bytes.extend(b":[");
    for (index, page) in pages.iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        bytes.extend(b"{\"page\":");
        bytes.extend(serde_json::to_vec(&page.page)?);
        bytes.extend(b",\"response\":");
        write_canonical_json_without_lf(bytes, &page.response)?;
        bytes.push(b'}');
    }
    bytes.push(b']');
    Ok(())
}

fn write_requested_cards(
    bytes: &mut Vec<u8>,
    name: &str,
    cards: &[RequestedCardResponse],
) -> Result<(), RawContractError> {
    bytes.extend(serde_json::to_vec(name)?);
    bytes.extend(b":[");
    for (index, card) in cards.iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        bytes.extend(b"{\"requested_card_id\":");
        bytes.extend(serde_json::to_vec(&card.requested_card_id)?);
        bytes.extend(b",\"response\":");
        write_canonical_json_without_lf(bytes, &card.response)?;
        bytes.push(b'}');
    }
    bytes.push(b']');
    Ok(())
}

fn write_canonical_json_without_lf(
    bytes: &mut Vec<u8>,
    value: &Value,
) -> Result<(), RawContractError> {
    let mut canonical = canonical_json_bytes(value)?;
    canonical.pop();
    bytes.extend(canonical);
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{CardListQuery, RawMetadata, RawPage, RawSnapshot, RawSource};

    #[test]
    fn raw_snapshot_keeps_wrapper_order_and_sorts_only_response_objects() {
        let snapshot = RawSnapshot {
            format_version: 1,
            source: RawSource::blizzard_us(),
            collected_at: "2026-08-05T00:00:00Z".into(),
            query: CardListQuery::standard("ko_KR"),
            card_pages: vec![RawPage {
                page: 1,
                response: json!({"z": 1, "a": {"y": 2, "b": 3}}),
            }],
            related_cards: vec![],
            class_reference_cards: vec![],
            metadata: RawMetadata {
                response: json!({"sets": []}),
            },
        };

        assert_eq!(
            String::from_utf8(snapshot.canonical_bytes().unwrap()).unwrap(),
            "{\"format_version\":1,\"source\":{\"provider\":\"blizzard\",\"api\":\"hearthstone_game_data\",\"region\":\"us\",\"endpoints\":{\"cards\":\"https://us.api.blizzard.com/hearthstone/cards\",\"card_by_id_template\":\"https://us.api.blizzard.com/hearthstone/cards/{card-id}\",\"metadata\":\"https://us.api.blizzard.com/hearthstone/metadata\"}},\"collected_at\":\"2026-08-05T00:00:00Z\",\"query\":{\"locale\":\"ko_KR\",\"set\":\"standard\",\"gameMode\":\"constructed\",\"collectible\":\"0,1\",\"pageSize\":500},\"card_pages\":[{\"page\":1,\"response\":{\"a\":{\"b\":3,\"y\":2},\"z\":1}}],\"related_cards\":[],\"class_reference_cards\":[],\"metadata\":{\"response\":{\"sets\":[]}}}\n"
        );
    }

    #[test]
    fn raw_snapshot_rejects_secret_shaped_response_fields() {
        let snapshot = RawSnapshot {
            format_version: 1,
            source: RawSource::blizzard_us(),
            collected_at: "2026-08-05T00:00:00Z".into(),
            query: CardListQuery::standard("en_US"),
            card_pages: vec![],
            related_cards: vec![],
            class_reference_cards: vec![],
            metadata: RawMetadata {
                response: json!({"access_token": "do-not-store"}),
            },
        };

        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn raw_snapshot_rejects_camel_case_and_hyphenated_secret_keys() {
        for key in [
            "clientId",
            "client-id",
            "clientSecret",
            "accessToken",
            "refreshToken",
            "authorization",
            "oauth",
        ] {
            let snapshot = RawSnapshot {
                format_version: 1,
                source: RawSource::blizzard_us(),
                collected_at: "2026-08-05T00:00:00Z".into(),
                query: CardListQuery::standard("en_US"),
                card_pages: vec![],
                related_cards: vec![],
                class_reference_cards: vec![],
                metadata: RawMetadata {
                    response: json!({key: "do-not-store"}),
                },
            };

            assert!(snapshot.validate().is_err(), "accepted {key}");
        }
    }
}
