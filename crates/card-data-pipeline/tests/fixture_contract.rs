use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use card_data_contract::official::{CardsPageResponse, MetadataResponse, OfficialCard};
use serde_json::Value;

const CARD_IDS: [i64; 12] = [
    1001, 1002, 1003, 1004, 1005, 1006, 1007, 2001, 2002, 2003, 3001, 3002,
];

struct FixtureLocale {
    cards: Vec<OfficialCard>,
    metadata: MetadataResponse,
    documents: Vec<Value>,
}

impl FixtureLocale {
    fn load(locale: &str) -> Self {
        let root = fixture_root().join(locale);
        let page_value = read_json(&root.join("cards-page-1.json"));
        let page: CardsPageResponse = serde_json::from_value(page_value.clone())
            .expect("list fixture must deserialize as the official response type");
        assert_eq!(page.card_count, 7);
        assert_eq!(page.page_count, 1);
        assert_eq!(page.page, 1);
        assert_eq!(
            page.cards
                .iter()
                .map(|card| card.id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([1001, 1002, 1003, 1004, 1005, 1006, 1007]),
        );

        let metadata_value = read_json(&root.join("metadata.json"));
        let metadata: MetadataResponse = serde_json::from_value(metadata_value.clone())
            .expect("metadata fixture must deserialize as the official response type");
        assert!(!metadata.sets.is_empty());
        assert!(!metadata.classes.is_empty());
        assert!(!metadata.types.is_empty());
        assert!(!metadata.rarities.is_empty());
        assert!(!metadata.minion_types.is_empty());
        assert!(!metadata.spell_schools.is_empty());
        assert!(!metadata.keywords.is_empty());
        assert_eq!(
            metadata
                .rarities
                .iter()
                .find(|rarity| rarity.id == 2)
                .expect("nullable-currency rarity")
                .crafting_cost,
            Some([None, Some(400)])
        );

        let mut cards = page.cards;
        let mut documents = vec![page_value, metadata_value];
        for id in [2001, 2002, 2003, 3001, 3002] {
            let value = read_json(&root.join("cards").join(format!("{id}.json")));
            let card: OfficialCard = serde_json::from_value(value.clone())
                .expect("card fixture must deserialize as the official response type");
            assert_eq!(card.id, id);
            cards.push(card);
            documents.push(value);
        }

        Self {
            cards,
            metadata,
            documents,
        }
    }

    fn card_ids(&self) -> BTreeSet<i64> {
        self.cards.iter().map(|card| card.id).collect()
    }

    fn card(&self, id: i64) -> &OfficialCard {
        self.cards
            .iter()
            .find(|card| card.id == id)
            .expect("fixture card must exist")
    }

    fn cards(&self) -> impl Iterator<Item = &OfficialCard> {
        self.cards.iter()
    }

    fn class(&self, id: i64) -> &card_data_contract::official::OfficialClass {
        self.metadata
            .classes
            .iter()
            .find(|class| class.id == id)
            .expect("fixture class must exist")
    }
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/fixtures/card-data-pipeline/v1")
}

fn read_json(path: &Path) -> Value {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("fixture {} must exist: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("fixture {} must be valid JSON: {error}", path.display()))
}

fn structural_value(mut value: Value) -> Value {
    match &mut value {
        Value::Array(values) => {
            for value in values {
                *value = structural_value(value.take());
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if matches!(
                    key.as_str(),
                    "name"
                        | "text"
                        | "flavorText"
                        | "artistName"
                        | "image"
                        | "cropImage"
                        | "refText"
                ) {
                    *value = Value::Null;
                } else {
                    *value = structural_value(value.take());
                }
            }
        }
        _ => {}
    }
    value
}

#[test]
fn canonical_two_locale_fixture_covers_collection_and_normalization_cases() {
    let ko = FixtureLocale::load("ko_KR");
    let en = FixtureLocale::load("en_US");

    assert_eq!(ko.card_ids(), en.card_ids());
    assert_eq!(ko.card_ids(), BTreeSet::from(CARD_IDS));
    assert!(ko.card(1001).child_ids.contains(&2001));
    assert!(ko.card(1001).bundled_card_ids.contains(&2002));
    assert!(ko.card(2001).child_ids.contains(&2003));
    assert_eq!(ko.card(1001).parent_id, Some(9999));
    assert_eq!(ko.card(1001).copy_of_card_id, vec![9998]);
    assert_eq!(ko.class(1).card_id, Some(3001));
    assert_eq!(ko.class(1).hero_power_card_id, Some(3002));
    assert_eq!(ko.card(1001).card_type_id, 4, "normal minion");
    assert_eq!(ko.card(1002).card_type_id, 5, "spell");
    assert_eq!(ko.card(1003).card_type_id, 39, "location");
    assert_eq!(ko.card(1004).card_type_id, 7, "weapon");
    assert!(ko.card(1005).rune_cost.is_some(), "rune card");
    let sideboard = ko.card(1006).sideboard.as_ref().expect("sideboard card");
    assert_eq!(sideboard.max_cards, 3);
    assert!(!sideboard.ignores_class);
    assert!(sideboard.cards_count_as_max);
    assert_eq!(ko.card(1006).banned_from_sideboard, Some(true));
    assert_eq!(
        ko.metadata
            .rarities
            .iter()
            .find(|rarity| rarity.id == 2)
            .unwrap()
            .dust_value,
        Some([Some(5), None])
    );
    assert!(
        !ko.card(1007).multi_class_ids.is_empty() && !ko.card(1007).multi_type_ids.is_empty(),
        "multi-class/multi-type card",
    );
    assert!(ko.cards().any(|card| card.rune_cost.is_some()));
    assert!(ko.cards().any(|card| card.sideboard.is_some()));
    assert!(ko
        .cards()
        .any(|card| card.banned_from_sideboard == Some(true)));
    assert!(ko
        .cards()
        .any(|card| !card.multi_class_ids.is_empty() && !card.multi_type_ids.is_empty()));
    assert_eq!(
        ko.cards()
            .filter(|card| CARD_IDS[..7].contains(&card.id))
            .map(|card| card.card_type_id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([4, 5, 7, 39]),
    );
    assert_eq!(ko.card(1002).flavor_text.as_deref(), Some(""));
    assert_eq!(en.card(1003).name.as_deref(), Some("고대 요새"));
    assert_eq!(
        ko.documents
            .iter()
            .cloned()
            .map(structural_value)
            .collect::<Vec<_>>(),
        en.documents
            .iter()
            .cloned()
            .map(structural_value)
            .collect::<Vec<_>>(),
        "locale fixtures may differ only in localized strings and image URLs",
    );
}
