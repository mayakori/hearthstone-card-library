use std::path::Path;

use card_data_contract::normalized::{JoinSourceKind, ScopeKind};
use card_data_contract::{
    CardCounts, NormalizedCatalog, RelationKind, SourceField, SCHEMA_VERSION,
};
use rusqlite::{params, Connection, Transaction};

use crate::PipelineError;

const SCHEMA: &str = include_str!("schema.sql");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteBuildMetadata {
    pub schema_version: u32,
    pub data_version: String,
    pub locale: String,
    pub generated_at: String,
    pub source_raw_sha256: String,
    pub card_counts: CardCounts,
}

impl SqliteBuildMetadata {
    pub fn new(
        data_version: impl Into<String>,
        generated_at: impl Into<String>,
        catalog: &NormalizedCatalog,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            data_version: data_version.into(),
            locale: catalog.locale.clone(),
            generated_at: generated_at.into(),
            source_raw_sha256: catalog.source_raw_sha256.clone(),
            card_counts: catalog.card_counts,
        }
    }
}

pub struct SqliteWriter;

impl SqliteWriter {
    pub fn write(
        path: impl AsRef<Path>,
        catalog: &NormalizedCatalog,
        metadata: &SqliteBuildMetadata,
    ) -> Result<(), PipelineError> {
        catalog
            .validate()
            .map_err(|error| PipelineError::Normalize(error.to_string()))?;
        validate_metadata(catalog, metadata)?;
        let path = path.as_ref();
        if path.exists() {
            return Err(PipelineError::Sqlite(
                "SQLite output path already exists".into(),
            ));
        }
        let mut connection = Connection::open(path).map_err(sqlite_error)?;
        connection
            .execute_batch(
                "PRAGMA encoding = 'UTF-8';\
                 PRAGMA page_size = 4096;\
                 PRAGMA auto_vacuum = NONE;\
                 PRAGMA journal_mode = DELETE;\
                 PRAGMA foreign_keys = ON;\
                 PRAGMA synchronous = FULL;\
                 PRAGMA user_version = 1;",
            )
            .map_err(sqlite_error)?;
        let transaction = connection.transaction().map_err(sqlite_error)?;
        transaction.execute_batch(SCHEMA).map_err(sqlite_error)?;
        insert_catalog(&transaction, catalog, metadata)?;
        transaction.commit().map_err(sqlite_error)?;
        require_valid_database(&connection)?;
        Ok(())
    }
}

fn validate_metadata(
    catalog: &NormalizedCatalog,
    metadata: &SqliteBuildMetadata,
) -> Result<(), PipelineError> {
    if metadata.schema_version != SCHEMA_VERSION
        || metadata.locale != catalog.locale
        || metadata.generated_at != catalog.generated_at
        || metadata.source_raw_sha256 != catalog.source_raw_sha256
        || metadata.card_counts != catalog.card_counts
    {
        return Err(PipelineError::Sqlite(
            "SQLite metadata does not match normalized catalog".into(),
        ));
    }
    Ok(())
}

fn insert_catalog(
    transaction: &Transaction<'_>,
    catalog: &NormalizedCatalog,
    metadata: &SqliteBuildMetadata,
) -> Result<(), PipelineError> {
    transaction
        .execute(
            "INSERT INTO catalog_metadata VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                1,
                i64::from(metadata.schema_version),
                metadata.data_version,
                metadata.locale,
                metadata.generated_at,
                metadata.source_raw_sha256,
                metadata.card_counts.standard as i64,
                metadata.card_counts.related as i64,
                metadata.card_counts.class_reference as i64,
                metadata.card_counts.total as i64,
            ],
        )
        .map_err(sqlite_error)?;
    for row in &catalog.sets {
        transaction
            .execute(
                "INSERT INTO sets VALUES (?1, ?2, ?3)",
                params![row.id, row.slug, row.name],
            )
            .map_err(sqlite_error)?;
    }
    for row in &catalog.card_types {
        transaction
            .execute(
                "INSERT INTO card_types VALUES (?1, ?2, ?3)",
                params![row.id, row.slug, row.name],
            )
            .map_err(sqlite_error)?;
    }
    for row in &catalog.rarities {
        transaction
            .execute(
                "INSERT INTO rarities VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    row.id,
                    row.slug,
                    row.name,
                    row.crafting_cost_normal,
                    row.crafting_cost_golden,
                    row.dust_value_normal,
                    row.dust_value_golden
                ],
            )
            .map_err(sqlite_error)?;
    }
    for row in &catalog.minion_types {
        transaction
            .execute(
                "INSERT INTO minion_types VALUES (?1, ?2, ?3)",
                params![row.id, row.slug, row.name],
            )
            .map_err(sqlite_error)?;
    }
    for row in &catalog.spell_schools {
        transaction
            .execute(
                "INSERT INTO spell_schools VALUES (?1, ?2, ?3)",
                params![row.id, row.slug, row.name],
            )
            .map_err(sqlite_error)?;
    }
    for row in &catalog.keywords {
        transaction
            .execute(
                "INSERT INTO keywords VALUES (?1, ?2, ?3, ?4, ?5)",
                params![row.id, row.slug, row.name, row.ref_text, row.text],
            )
            .map_err(sqlite_error)?;
    }
    for row in &catalog.cards {
        transaction.execute(
            "INSERT INTO cards VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30)",
            params![row.id, row.slug, scope_kind(row.scope_kind), bool_int(row.collectible), row.name, row.text_markup, row.text_plain, row.flavor_text, row.artist_name, row.mana_cost, row.attack, row.health, row.armor, row.deck_size_mod, row.set_id, row.type_id, row.rarity_id, row.spell_school_id, row.image_url, row.crop_image_url, row.rune_blood, row.rune_frost, row.rune_unholy, row.sideboard_max_cards, row.sideboard_subset, row.sideboard_ignores_class.map(bool_int), row.sideboard_cards_count_as_max.map(bool_int), bool_int(row.banned_from_sideboard), bool_int(row.zilliax_functional_module), bool_int(row.zilliax_cosmetic_module)],
        ).map_err(sqlite_error)?;
    }
    for row in &catalog.classes {
        transaction
            .execute(
                "INSERT INTO classes VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    row.id,
                    row.slug,
                    row.name,
                    row.default_hero_card_id,
                    row.default_hero_power_card_id
                ],
            )
            .map_err(sqlite_error)?;
    }
    for row in &catalog.card_classes {
        transaction
            .execute(
                "INSERT INTO card_classes VALUES (?1, ?2, ?3, ?4)",
                params![
                    row.card_id,
                    row.taxonomy_id,
                    i64::from(row.position),
                    join_kind(row.source_kind)
                ],
            )
            .map_err(sqlite_error)?;
    }
    for row in &catalog.card_minion_types {
        transaction
            .execute(
                "INSERT INTO card_minion_types VALUES (?1, ?2, ?3, ?4)",
                params![
                    row.card_id,
                    row.taxonomy_id,
                    i64::from(row.position),
                    join_kind(row.source_kind)
                ],
            )
            .map_err(sqlite_error)?;
    }
    for row in &catalog.card_keywords {
        transaction
            .execute(
                "INSERT INTO card_keywords VALUES (?1, ?2, ?3)",
                params![row.card_id, row.keyword_id, i64::from(row.position)],
            )
            .map_err(sqlite_error)?;
    }
    for row in &catalog.relations {
        transaction
            .execute(
                "INSERT INTO card_relations VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    row.source_card_id,
                    relation_kind(row.relation_kind),
                    source_field(row.source_field),
                    row.target_card_id,
                    i64::from(row.display_order)
                ],
            )
            .map_err(sqlite_error)?;
    }
    Ok(())
}

fn require_valid_database(connection: &Connection) -> Result<(), PipelineError> {
    let mut statement = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(sqlite_error)?;
    let mut rows = statement.query([]).map_err(sqlite_error)?;
    if rows.next().map_err(sqlite_error)?.is_some() {
        return Err(PipelineError::Sqlite("foreign_key_check failed".into()));
    }
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(sqlite_error)?;
    if integrity != "ok" {
        return Err(PipelineError::Sqlite("integrity_check failed".into()));
    }
    Ok(())
}

fn bool_int(value: bool) -> i64 {
    i64::from(value)
}
fn scope_kind(value: ScopeKind) -> &'static str {
    match value {
        ScopeKind::Standard => "standard",
        ScopeKind::ClassReference => "class_reference",
        ScopeKind::Related => "related",
    }
}
fn join_kind(value: JoinSourceKind) -> &'static str {
    match value {
        JoinSourceKind::Primary => "primary",
        JoinSourceKind::Multi => "multi",
    }
}
fn relation_kind(value: RelationKind) -> &'static str {
    match value {
        RelationKind::Child => "child",
        RelationKind::Bundled => "bundled",
        RelationKind::Parent => "parent",
        RelationKind::CopyOf => "copy_of",
    }
}
fn source_field(value: SourceField) -> &'static str {
    match value {
        SourceField::ChildIds => "childIds",
        SourceField::BundledCardIds => "bundledCardIds",
        SourceField::ParentId => "parentId",
        SourceField::CopyOfCardId => "copyOfCardId",
    }
}
fn sqlite_error(error: rusqlite::Error) -> PipelineError {
    PipelineError::Sqlite(error.to_string())
}
