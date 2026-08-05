use std::collections::BTreeMap;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficialCard {
    pub id: i64,
    pub slug: String,
    pub collectible: i64,
    #[serde(rename = "manaCost")]
    pub mana_cost: i64,
    #[serde(rename = "cardSetId")]
    pub card_set_id: i64,
    #[serde(rename = "cardTypeId")]
    pub card_type_id: i64,
    #[serde(rename = "classId")]
    pub class_id: Option<i64>,
    #[serde(rename = "multiClassIds", default)]
    pub multi_class_ids: Vec<i64>,
    #[serde(rename = "minionTypeId")]
    pub minion_type_id: Option<i64>,
    #[serde(rename = "multiTypeIds", default)]
    pub multi_type_ids: Vec<i64>,
    #[serde(rename = "rarityId")]
    pub rarity_id: Option<i64>,
    #[serde(rename = "spellSchoolId")]
    pub spell_school_id: Option<i64>,
    #[serde(rename = "keywordIds", default)]
    pub keyword_ids: Vec<i64>,
    pub name: Option<String>,
    pub text: Option<String>,
    #[serde(rename = "flavorText")]
    pub flavor_text: Option<String>,
    #[serde(rename = "artistName")]
    pub artist_name: Option<String>,
    pub image: Option<String>,
    #[serde(rename = "cropImage")]
    pub crop_image: Option<String>,
    pub attack: Option<i64>,
    pub health: Option<i64>,
    pub armor: Option<i64>,
    #[serde(rename = "deckSize")]
    pub deck_size: Option<i64>,
    #[serde(rename = "childIds", default)]
    pub child_ids: Vec<i64>,
    #[serde(rename = "bundledCardIds", default)]
    pub bundled_card_ids: Vec<i64>,
    #[serde(rename = "parentId")]
    pub parent_id: Option<i64>,
    #[serde(rename = "copyOfCardId", default)]
    pub copy_of_card_id: Vec<i64>,
    #[serde(rename = "runeCost")]
    pub rune_cost: Option<RuneCost>,
    pub sideboard: Option<Sideboard>,
    #[serde(
        rename = "bannedFromSideboard",
        default,
        deserialize_with = "deserialize_optional_official_bool"
    )]
    pub banned_from_sideboard: Option<bool>,
    #[serde(rename = "isZilliaxFunctionalModule")]
    pub is_zilliax_functional_module: Option<bool>,
    #[serde(rename = "isZilliaxCosmeticModule")]
    pub is_zilliax_cosmetic_module: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuneCost {
    pub blood: i64,
    pub frost: i64,
    pub unholy: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sideboard {
    #[serde(rename = "maxSideboardCards")]
    pub max_cards: i64,
    #[serde(rename = "sideboardSubset")]
    pub sideboard_subset: String,
    #[serde(rename = "sideboardIgnoresClass")]
    pub ignores_class: bool,
    #[serde(rename = "sideboardCardsCountAsMax")]
    pub cards_count_as_max: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OfficialBool {
    Boolean(bool),
    Integer(i64),
}

fn deserialize_optional_official_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<OfficialBool>::deserialize(deserializer)? {
        None => Ok(None),
        Some(OfficialBool::Boolean(value)) => Ok(Some(value)),
        Some(OfficialBool::Integer(0)) => Ok(Some(false)),
        Some(OfficialBool::Integer(1)) => Ok(Some(true)),
        Some(OfficialBool::Integer(_)) => Err(D::Error::custom(
            "bannedFromSideboard must be boolean or zero/one",
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardsPageResponse {
    pub cards: Vec<OfficialCard>,
    #[serde(rename = "cardCount")]
    pub card_count: i64,
    #[serde(rename = "pageCount")]
    pub page_count: i64,
    pub page: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataResponse {
    pub sets: Vec<MetadataEntry>,
    pub classes: Vec<OfficialClass>,
    pub types: Vec<MetadataEntry>,
    pub rarities: Vec<OfficialRarity>,
    #[serde(rename = "minionTypes")]
    pub minion_types: Vec<MetadataEntry>,
    #[serde(rename = "spellSchools")]
    pub spell_schools: Vec<MetadataEntry>,
    pub keywords: Vec<OfficialKeyword>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub id: i64,
    pub slug: Option<String>,
    pub name: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficialClass {
    pub id: i64,
    pub slug: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "cardId")]
    pub card_id: Option<i64>,
    #[serde(rename = "heroPowerCardId")]
    pub hero_power_card_id: Option<i64>,
    #[serde(rename = "alternateHeroCardIds", default)]
    pub alternate_hero_card_ids: Vec<i64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficialRarity {
    pub id: i64,
    pub slug: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "craftingCost")]
    pub crafting_cost: Option<[Option<i64>; 2]>,
    #[serde(rename = "dustValue")]
    pub dust_value: Option<[Option<i64>; 2]>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficialKeyword {
    pub id: i64,
    pub slug: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "refText")]
    pub ref_text: Option<String>,
    pub text: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{MetadataResponse, OfficialCard};

    #[test]
    fn metadata_deserializes_the_official_array_currency_amounts() {
        let metadata = serde_json::from_value::<MetadataResponse>(json!({
            "sets": [],
            "classes": [],
            "types": [],
            "rarities": [{
                "id": 1,
                "slug": "common",
                "name": "Common",
                "craftingCost": [40, 400],
                "dustValue": [5, 50]
            }],
            "minionTypes": [],
            "spellSchools": [],
            "keywords": []
        }))
        .unwrap();

        let rarity = &metadata.rarities[0];
        let _: Option<[Option<i64>; 2]> = rarity.crafting_cost;
        let _: Option<[Option<i64>; 2]> = rarity.dust_value;
        assert_eq!(rarity.crafting_cost, Some([Some(40), Some(400)]));
        assert_eq!(rarity.dust_value, Some([Some(5), Some(50)]));
    }

    #[test]
    fn card_deserializes_current_sideboard_wire_keys_and_boolean_flag() {
        let card = serde_json::from_value::<OfficialCard>(json!({
            "id": 1006,
            "slug": "sideboard-card",
            "collectible": 1,
            "manaCost": 6,
            "cardSetId": 1,
            "cardTypeId": 5,
            "sideboard": {
                "maxSideboardCards": 3,
                "sideboardSubset": "fixture",
                "sideboardIgnoresClass": false,
                "sideboardCardsCountAsMax": true
            },
            "bannedFromSideboard": true
        }))
        .expect("current official sideboard schema");

        let sideboard = card.sideboard.expect("sideboard object");
        assert_eq!(sideboard.max_cards, 3);
        assert!(!sideboard.ignores_class);
        assert!(sideboard.cards_count_as_max);
        assert_eq!(card.banned_from_sideboard, Some(true));
    }

    #[test]
    fn card_normalizes_numeric_sideboard_flag_to_boolean() {
        let card = serde_json::from_value::<OfficialCard>(json!({
            "id": 1007,
            "slug": "numeric-sideboard-flag",
            "collectible": 1,
            "manaCost": 1,
            "cardSetId": 1,
            "cardTypeId": 5,
            "bannedFromSideboard": 1
        }))
        .expect("numeric official sideboard flag");

        assert_eq!(card.banned_from_sideboard, Some(true));
    }

    #[test]
    fn metadata_preserves_nullable_currency_amounts() {
        let metadata = serde_json::from_value::<MetadataResponse>(json!({
            "sets": [], "classes": [], "types": [],
            "rarities": [{
                "id": 2,
                "craftingCost": [null, 400],
                "dustValue": [5, null]
            }],
            "minionTypes": [], "spellSchools": [], "keywords": []
        }))
        .expect("nullable official currency amounts");

        assert_eq!(metadata.rarities[0].crafting_cost, Some([None, Some(400)]));
        assert_eq!(metadata.rarities[0].dust_value, Some([Some(5), None]));
    }
}
