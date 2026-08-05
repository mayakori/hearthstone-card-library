use std::{collections::BTreeMap, fs, path::Path};

use card_data_contract::normalized::ScopeKind;
use card_data_contract::{
    official::{MetadataResponse, OfficialCard},
    raw::{CardListQuery, RawMetadata, RawSnapshot, RawSource},
    RelationKind, SourceField,
};
use card_data_pipeline::{normalize_locale, CollectedLocale, SqliteBuildMetadata, SqliteWriter};
use serde_json::{json, Value};

const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn fixture(path: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/fixtures/card-data-pipeline/v1/ko_KR");
    serde_json::from_str(&fs::read_to_string(root.join(path)).unwrap()).unwrap()
}

#[test]
fn sqlite_schema_is_an_exact_copy_of_the_approved_ddl() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = fs::read_to_string(manifest.join(
        "../../docs/superpowers/specs/2026-07-25-hcl-006-official-card-data-pipeline-design.md",
    ))
    .unwrap()
    .replace("\r\n", "\n");
    let approved = spec
        .split_once("### 10.1 DDL\n\n```sql\n")
        .unwrap()
        .1
        .split_once("\n```\n")
        .unwrap()
        .0;
    let schema = fs::read_to_string(manifest.join("src/schema.sql"))
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(schema, format!("{approved}\n"));
}

fn locale() -> CollectedLocale {
    let page = fixture("cards-page-1.json");
    let cards = page["cards"].as_array().unwrap();
    let mut standard_cards = BTreeMap::new();
    for card in cards {
        let card: OfficialCard = serde_json::from_value(card.clone()).unwrap();
        standard_cards.insert(card.id, card);
    }
    let mut related_cards = BTreeMap::new();
    for id in [2001, 2002, 2003] {
        let card: OfficialCard =
            serde_json::from_value(fixture(&format!("cards/{id}.json"))).unwrap();
        related_cards.insert(id, card);
    }
    let mut class_reference_cards = BTreeMap::new();
    for id in [3001, 3002] {
        let card: OfficialCard =
            serde_json::from_value(fixture(&format!("cards/{id}.json"))).unwrap();
        class_reference_cards.insert(id, card);
    }
    CollectedLocale {
        locale: "ko_KR".into(),
        standard_cards,
        related_cards,
        class_reference_cards,
        metadata: serde_json::from_value::<MetadataResponse>(fixture("metadata.json")).unwrap(),
        raw: RawSnapshot {
            format_version: 1,
            source: RawSource::blizzard_us(),
            collected_at: "2026-08-05T00:00:00Z".into(),
            query: CardListQuery::standard("ko_KR"),
            card_pages: vec![],
            related_cards: vec![],
            class_reference_cards: vec![],
            metadata: RawMetadata {
                response: Value::Null,
            },
        },
    }
}

#[test]
fn normalizes_official_fields_without_locale_fallback_and_keeps_dangling_parent() {
    let catalog = normalize_locale(&locale(), SHA256, "2026-08-05T00:00:00Z").unwrap();
    let spell = catalog.cards.iter().find(|card| card.id == 1002).unwrap();
    assert_eq!(spell.flavor_text, None, "empty strings become NULL");
    assert_eq!(spell.text_markup.as_deref(), Some("카드를 뽑습니다."));
    assert_eq!(spell.text_plain.as_deref(), Some("카드를 뽑습니다."));
    assert_eq!(
        spell.image_url.as_deref(),
        Some("https://ko.example.test/1002.png")
    );
    let rune = catalog.cards.iter().find(|card| card.id == 1005).unwrap();
    assert_eq!(
        (rune.rune_blood, rune.rune_frost, rune.rune_unholy),
        (Some(1), Some(0), Some(0))
    );
    let sideboard = catalog.cards.iter().find(|card| card.id == 1006).unwrap();
    assert_eq!(sideboard.sideboard_max_cards, Some(3));
    assert!(sideboard.banned_from_sideboard);
    assert!(catalog
        .relations
        .iter()
        .any(|relation| relation.relation_kind == RelationKind::Parent
            && relation.source_field == SourceField::ParentId
            && relation.target_card_id == 9999));
    assert_eq!(
        catalog
            .relations
            .iter()
            .map(|r| r.target_card_id)
            .collect::<Vec<_>>(),
        vec![2002, 2001, 9998, 9999, 2003]
    );
    assert!(catalog
        .cards
        .iter()
        .all(|card| card.image_url.as_deref() != Some("gold-only-image")));
    assert_eq!(
        catalog
            .relations
            .iter()
            .find(|relation| relation.target_card_id == 2001)
            .unwrap()
            .source_field,
        SourceField::ChildIds
    );
}

#[test]
fn keeps_dangling_class_reference_children_without_collecting_hero_skins() {
    let mut collected = locale();
    collected.metadata.classes[0].alternate_hero_card_ids = vec![3999];
    collected
        .class_reference_cards
        .get_mut(&3001)
        .unwrap()
        .child_ids = vec![3999];

    let catalog = normalize_locale(&collected, SHA256, "2026-08-05T00:00:00Z").unwrap();

    assert!(!catalog.cards.iter().any(|card| card.id == 3999));
    assert!(catalog.relations.iter().any(|relation| {
        relation.source_card_id == 3001
            && relation.relation_kind == RelationKind::Child
            && relation.source_field == SourceField::ChildIds
            && relation.target_card_id == 3999
    }));
}

#[test]
fn rejects_dangling_non_skin_children_from_class_references() {
    let mut collected = locale();
    collected
        .class_reference_cards
        .get_mut(&3001)
        .unwrap()
        .child_ids = vec![3999];

    assert!(normalize_locale(&collected, SHA256, "2026-08-05T00:00:00Z").is_err());
}

#[test]
fn preserves_nonempty_markup_and_opaque_empty_sideboard_subset() {
    let mut collected = locale();
    let card = collected.standard_cards.get_mut(&1001).unwrap();
    card.text = Some("<b></b>".into());
    card.sideboard = Some(card_data_contract::official::Sideboard {
        max_cards: 0,
        sideboard_subset: String::new(),
        ignores_class: false,
        cards_count_as_max: false,
    });
    let catalog = normalize_locale(&collected, SHA256, "2026-08-05T00:00:00Z").unwrap();
    let card = catalog.cards.iter().find(|card| card.id == 1001).unwrap();
    assert_eq!(card.text_markup.as_deref(), Some("<b></b>"));
    assert_eq!(card.text_plain.as_deref(), Some(""));
    assert_eq!(card.sideboard_subset.as_deref(), Some(""));
}

#[test]
fn applies_scope_precedence_and_rejects_invalid_official_values() {
    let mut scoped = locale();
    scoped
        .related_cards
        .insert(1001, scoped.standard_cards[&1001].clone());
    let catalog = normalize_locale(&scoped, SHA256, "2026-08-05T00:00:00Z").unwrap();
    assert_eq!(catalog.card_counts.total, 12);
    assert_eq!(catalog.card_counts.standard, 7);
    assert_eq!(
        catalog
            .cards
            .iter()
            .find(|card| card.id == 1001)
            .unwrap()
            .scope_kind,
        ScopeKind::Standard
    );

    let mut negative = locale();
    negative.standard_cards.get_mut(&1001).unwrap().mana_cost = -1;
    assert!(normalize_locale(&negative, SHA256, "2026-08-05T00:00:00Z").is_err());

    let mut duplicate = locale();
    duplicate
        .standard_cards
        .get_mut(&1001)
        .unwrap()
        .keyword_ids
        .push(1);
    assert!(normalize_locale(&duplicate, SHA256, "2026-08-05T00:00:00Z").is_err());

    let mut dangling_forward = locale();
    dangling_forward
        .standard_cards
        .get_mut(&1001)
        .unwrap()
        .child_ids = vec![4040];
    assert!(normalize_locale(&dangling_forward, SHA256, "2026-08-05T00:00:00Z").is_err());

    let mut placeholders = locale();
    let card = placeholders.standard_cards.get_mut(&1002).unwrap();
    card.card_set_id = 91;
    card.card_type_id = 92;
    card.rarity_id = Some(93);
    card.spell_school_id = Some(94);
    card.class_id = Some(95);
    card.minion_type_id = Some(96);
    card.keyword_ids = vec![97];
    let catalog = normalize_locale(&placeholders, SHA256, "2026-08-05T00:00:00Z").unwrap();
    assert!(catalog
        .sets
        .iter()
        .any(|row| row.id == 91 && row.slug.is_none()));
    assert!(catalog
        .card_types
        .iter()
        .any(|row| row.id == 92 && row.name.is_none()));
    assert!(catalog
        .rarities
        .iter()
        .any(|row| row.id == 93 && row.name.is_none()));
    assert!(catalog
        .spell_schools
        .iter()
        .any(|row| row.id == 94 && row.name.is_none()));
    assert!(catalog
        .classes
        .iter()
        .any(|row| row.id == 95 && row.name.is_none()));
    assert!(catalog
        .minion_types
        .iter()
        .any(|row| row.id == 96 && row.name.is_none()));
    assert!(catalog
        .keywords
        .iter()
        .any(|row| row.id == 97 && row.name.is_none()));

    let mut duplicate_multi_class = locale();
    duplicate_multi_class
        .standard_cards
        .get_mut(&1007)
        .unwrap()
        .multi_class_ids = vec![2, 2];
    assert!(normalize_locale(&duplicate_multi_class, SHA256, "2026-08-05T00:00:00Z").is_err());
    let mut duplicate_multi_type = locale();
    duplicate_multi_type
        .standard_cards
        .get_mut(&1007)
        .unwrap()
        .multi_type_ids = vec![21, 21];
    assert!(normalize_locale(&duplicate_multi_type, SHA256, "2026-08-05T00:00:00Z").is_err());
    let mut duplicate_bundled = locale();
    duplicate_bundled
        .standard_cards
        .get_mut(&1001)
        .unwrap()
        .bundled_card_ids = vec![2002, 2002];
    assert!(normalize_locale(&duplicate_bundled, SHA256, "2026-08-05T00:00:00Z").is_err());
    let mut duplicate_copy = locale();
    duplicate_copy
        .standard_cards
        .get_mut(&1001)
        .unwrap()
        .copy_of_card_id = vec![9998, 9998];
    assert!(normalize_locale(&duplicate_copy, SHA256, "2026-08-05T00:00:00Z").is_err());

    let mut negative_rune = locale();
    negative_rune
        .standard_cards
        .get_mut(&1005)
        .unwrap()
        .rune_cost
        .as_mut()
        .unwrap()
        .blood = -1;
    assert!(normalize_locale(&negative_rune, SHA256, "2026-08-05T00:00:00Z").is_err());
    let mut negative_sideboard = locale();
    negative_sideboard
        .standard_cards
        .get_mut(&1006)
        .unwrap()
        .sideboard
        .as_mut()
        .unwrap()
        .max_cards = -1;
    assert!(normalize_locale(&negative_sideboard, SHA256, "2026-08-05T00:00:00Z").is_err());
    let mut negative_rarity = locale();
    negative_rarity.metadata.rarities[0].dust_value = Some([Some(-1), Some(50)]);
    assert!(normalize_locale(&negative_rarity, SHA256, "2026-08-05T00:00:00Z").is_err());

    let defaults = normalize_locale(&locale(), SHA256, "2026-08-05T00:00:00Z").unwrap();
    let default_card = defaults.cards.iter().find(|card| card.id == 1001).unwrap();
    assert!(!default_card.banned_from_sideboard);
    assert!(!default_card.zilliax_functional_module);
    assert!(!default_card.zilliax_cosmetic_module);
}

#[test]
fn rejects_partial_rune_and_sideboard_official_objects_at_the_input_boundary() {
    let collected = locale();
    for (card_id, object, missing_key) in [
        (1005, "runeCost", "blood"),
        (1006, "sideboard", "maxSideboardCards"),
    ] {
        let mut value = serde_json::to_value(&collected.standard_cards[&card_id]).unwrap();
        value[object].as_object_mut().unwrap().remove(missing_key);
        assert!(serde_json::from_value::<OfficialCard>(value).is_err());
    }
}

#[test]
fn writes_deterministic_complete_sqlite_catalog() {
    let mut collected = locale();
    collected
        .standard_cards
        .get_mut(&1001)
        .unwrap()
        .extra
        .insert(
            "imageGold".into(),
            json!("https://ko.example.test/1001-gold.png"),
        );
    let catalog = normalize_locale(&collected, SHA256, "2026-08-05T00:00:00Z").unwrap();
    let metadata =
        SqliteBuildMetadata::new("36.0.3-build247416-r1", "2026-08-05T00:00:00Z", &catalog);
    let root = std::env::temp_dir().join(format!("hcl-normalize-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let first = root.join("first.sqlite");
    let second = root.join("second.sqlite");
    SqliteWriter::write(&first, &catalog, &metadata).unwrap();
    let mut reordered = collected.clone();
    reordered.metadata.sets.reverse();
    reordered.metadata.classes.reverse();
    reordered.metadata.types.reverse();
    reordered.metadata.rarities.reverse();
    reordered.metadata.minion_types.reverse();
    reordered.metadata.spell_schools.reverse();
    reordered.metadata.keywords.reverse();
    let reordered_catalog = normalize_locale(&reordered, SHA256, "2026-08-05T00:00:00Z").unwrap();
    SqliteWriter::write(&second, &reordered_catalog, &metadata).unwrap();
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let connection = rusqlite::Connection::open(first).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        13
    );
    assert_eq!(connection.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name NOT LIKE 'sqlite_%'", [], |r| r.get::<_, i64>(0)).unwrap(), 5);
    assert_eq!(connection.query_row("SELECT standard_card_count, related_card_count, class_reference_card_count, total_card_count FROM catalog_metadata", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))).unwrap(), (7, 3, 2, 12));
    assert_eq!(connection.query_row("SELECT locale, data_version, generated_at, source_raw_sha256 FROM catalog_metadata", [], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))).unwrap(), ("ko_KR".into(), "36.0.3-build247416-r1".into(), "2026-08-05T00:00:00Z".into(), SHA256.into()));
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT rune_blood, rune_frost, rune_unholy FROM cards WHERE id = 1005",
                [],
                |r| Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?
                ))
            )
            .unwrap(),
        (1, 0, 0)
    );
    assert_eq!(connection.query_row("SELECT sideboard_max_cards, sideboard_subset, sideboard_ignores_class, sideboard_cards_count_as_max, banned_from_sideboard FROM cards WHERE id = 1006", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?))).unwrap(), (3, "fixture".into(), 0, 1, 1));
    assert_eq!(connection.query_row("SELECT crafting_cost_normal, crafting_cost_golden, dust_value_normal, dust_value_golden FROM rarities WHERE id = 2", [], |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, Option<i64>>(2)?, r.get::<_, Option<i64>>(3)?))).unwrap(), (None, Some(400), Some(5), None));
    assert_eq!(connection.query_row("SELECT relation_kind, source_field, target_card_id, display_order FROM card_relations WHERE source_card_id = 1001 ORDER BY relation_kind, display_order", [], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))).unwrap(), ("bundled".into(), "bundledCardIds".into(), 2002, 0));
    assert_eq!(
        connection
            .query_row("SELECT image_url FROM cards WHERE id = 1001", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
        "https://ko.example.test/1001.png"
    );
    assert_eq!(connection.query_row("SELECT count(*) FROM sqlite_master WHERE lower(name) LIKE '%fts%' OR lower(sql) LIKE '%name_choseong%'", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert_eq!(
        connection
            .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    drop(connection);
    fs::remove_dir_all(root).unwrap();
}
