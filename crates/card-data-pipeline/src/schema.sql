CREATE TABLE catalog_metadata (
  singleton                  INTEGER PRIMARY KEY CHECK (singleton = 1),
  schema_version             INTEGER NOT NULL CHECK (schema_version > 0),
  data_version               TEXT NOT NULL,
  locale                     TEXT NOT NULL CHECK (locale IN ('ko_KR', 'en_US')),
  generated_at               TEXT NOT NULL,
  source_raw_sha256          TEXT NOT NULL CHECK (
    length(source_raw_sha256) = 64 AND
    source_raw_sha256 = lower(source_raw_sha256) AND
    source_raw_sha256 NOT GLOB '*[^0-9a-f]*'
  ),
  standard_card_count        INTEGER NOT NULL CHECK (standard_card_count >= 0),
  related_card_count         INTEGER NOT NULL CHECK (related_card_count >= 0),
  class_reference_card_count INTEGER NOT NULL CHECK (class_reference_card_count >= 0),
  total_card_count           INTEGER NOT NULL CHECK (
    total_card_count = standard_card_count + related_card_count + class_reference_card_count
  )
) STRICT;

CREATE TABLE sets (
  id   INTEGER PRIMARY KEY,
  slug TEXT,
  name TEXT
) STRICT;

CREATE TABLE card_types (
  id   INTEGER PRIMARY KEY,
  slug TEXT,
  name TEXT
) STRICT;

CREATE TABLE rarities (
  id                    INTEGER PRIMARY KEY,
  slug                  TEXT,
  name                  TEXT,
  crafting_cost_normal  INTEGER CHECK (crafting_cost_normal >= 0),
  crafting_cost_golden  INTEGER CHECK (crafting_cost_golden >= 0),
  dust_value_normal     INTEGER CHECK (dust_value_normal >= 0),
  dust_value_golden     INTEGER CHECK (dust_value_golden >= 0)
) STRICT;

CREATE TABLE minion_types (
  id   INTEGER PRIMARY KEY,
  slug TEXT,
  name TEXT
) STRICT;

CREATE TABLE spell_schools (
  id   INTEGER PRIMARY KEY,
  slug TEXT,
  name TEXT
) STRICT;

CREATE TABLE keywords (
  id       INTEGER PRIMARY KEY,
  slug     TEXT,
  name     TEXT,
  ref_text TEXT,
  text     TEXT
) STRICT;

CREATE TABLE cards (
  id                           INTEGER PRIMARY KEY,
  slug                         TEXT NOT NULL,
  scope_kind                   TEXT NOT NULL CHECK (
    scope_kind IN ('standard', 'class_reference', 'related')
  ),
  collectible                  INTEGER NOT NULL CHECK (collectible IN (0, 1)),
  name                         TEXT,
  text_markup                  TEXT,
  text_plain                   TEXT,
  flavor_text                  TEXT,
  artist_name                  TEXT,
  mana_cost                    INTEGER NOT NULL CHECK (mana_cost >= 0),
  attack                       INTEGER,
  health                       INTEGER,
  armor                        INTEGER,
  deck_size_mod                INTEGER,
  set_id                       INTEGER NOT NULL REFERENCES sets(id),
  type_id                      INTEGER NOT NULL REFERENCES card_types(id),
  rarity_id                    INTEGER REFERENCES rarities(id),
  spell_school_id              INTEGER REFERENCES spell_schools(id),
  image_url                    TEXT,
  crop_image_url               TEXT,
  rune_blood                   INTEGER CHECK (rune_blood >= 0),
  rune_frost                   INTEGER CHECK (rune_frost >= 0),
  rune_unholy                  INTEGER CHECK (rune_unholy >= 0),
  sideboard_max_cards          INTEGER CHECK (sideboard_max_cards >= 0),
  sideboard_subset             TEXT,
  sideboard_ignores_class      INTEGER CHECK (sideboard_ignores_class IN (0, 1)),
  sideboard_cards_count_as_max INTEGER CHECK (sideboard_cards_count_as_max IN (0, 1)),
  banned_from_sideboard        INTEGER NOT NULL CHECK (banned_from_sideboard IN (0, 1)),
  zilliax_functional_module    INTEGER NOT NULL CHECK (zilliax_functional_module IN (0, 1)),
  zilliax_cosmetic_module      INTEGER NOT NULL CHECK (zilliax_cosmetic_module IN (0, 1)),
  CHECK (
    (rune_blood IS NULL AND rune_frost IS NULL AND rune_unholy IS NULL) OR
    (rune_blood IS NOT NULL AND rune_frost IS NOT NULL AND rune_unholy IS NOT NULL)
  ),
  CHECK (
    (sideboard_max_cards IS NULL AND sideboard_subset IS NULL AND
     sideboard_ignores_class IS NULL AND sideboard_cards_count_as_max IS NULL) OR
    (sideboard_max_cards IS NOT NULL AND sideboard_subset IS NOT NULL AND
     sideboard_ignores_class IS NOT NULL AND sideboard_cards_count_as_max IS NOT NULL)
  )
) STRICT;

CREATE TABLE classes (
  id                         INTEGER PRIMARY KEY,
  slug                       TEXT,
  name                       TEXT,
  default_hero_card_id       INTEGER REFERENCES cards(id) DEFERRABLE INITIALLY DEFERRED,
  default_hero_power_card_id INTEGER REFERENCES cards(id) DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE card_classes (
  card_id     INTEGER NOT NULL REFERENCES cards(id),
  class_id    INTEGER NOT NULL REFERENCES classes(id),
  position    INTEGER NOT NULL CHECK (position >= 0),
  source_kind TEXT NOT NULL CHECK (source_kind IN ('primary', 'multi')),
  PRIMARY KEY (card_id, position),
  UNIQUE (card_id, class_id)
) STRICT;

CREATE TABLE card_minion_types (
  card_id        INTEGER NOT NULL REFERENCES cards(id),
  minion_type_id INTEGER NOT NULL REFERENCES minion_types(id),
  position       INTEGER NOT NULL CHECK (position >= 0),
  source_kind    TEXT NOT NULL CHECK (source_kind IN ('primary', 'multi')),
  PRIMARY KEY (card_id, position),
  UNIQUE (card_id, minion_type_id)
) STRICT;

CREATE TABLE card_keywords (
  card_id    INTEGER NOT NULL REFERENCES cards(id),
  keyword_id INTEGER NOT NULL REFERENCES keywords(id),
  position   INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (card_id, position),
  UNIQUE (card_id, keyword_id)
) STRICT;

CREATE TABLE card_relations (
  source_card_id INTEGER NOT NULL REFERENCES cards(id),
  relation_kind  TEXT NOT NULL CHECK (
    relation_kind IN ('child', 'bundled', 'parent', 'copy_of')
  ),
  source_field   TEXT NOT NULL CHECK (
    source_field IN ('childIds', 'bundledCardIds', 'parentId', 'copyOfCardId')
  ),
  target_card_id INTEGER NOT NULL,
  display_order  INTEGER NOT NULL CHECK (display_order >= 0),
  PRIMARY KEY (source_card_id, relation_kind, display_order),
  UNIQUE (source_card_id, relation_kind, target_card_id),
  CHECK (
    (relation_kind = 'child'   AND source_field = 'childIds') OR
    (relation_kind = 'bundled' AND source_field = 'bundledCardIds') OR
    (relation_kind = 'parent'  AND source_field = 'parentId') OR
    (relation_kind = 'copy_of' AND source_field = 'copyOfCardId')
  )
) STRICT;

CREATE INDEX idx_cards_scope
  ON cards(scope_kind, id);
CREATE INDEX idx_card_classes_class
  ON card_classes(class_id, card_id);
CREATE INDEX idx_card_minion_types_type
  ON card_minion_types(minion_type_id, card_id);
CREATE INDEX idx_card_keywords_keyword
  ON card_keywords(keyword_id, card_id);
CREATE INDEX idx_card_relations_target
  ON card_relations(target_card_id, relation_kind, source_card_id);
