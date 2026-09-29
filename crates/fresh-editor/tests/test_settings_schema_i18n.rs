//! The Settings UI translates the config schema's labels through the locale
//! catalogs, and falls back to the schema's English wherever a catalog has
//! no entry:
//!
//! * a setting's name — `settings.field.<path>`
//! * a setting's description — `settings.field.<path>_desc`
//! * a category's name and description — `settings.category.<slug>[_desc]`
//! * a section header — `settings.section.<slug>`
//!
//! The schema is parsed once in English and once in Japanese, and every
//! label of the Japanese parse is checked against what the catalog says it
//! should be.

use fresh::view::settings::schema::{
    parse_schema, section_display_name, SettingCategory, SettingSchema, SettingType,
};

const SCHEMA_JSON: &str = include_str!("../plugins/config-schema.json");
const LOCALE: &str = "ja";

/// `"Syntax & Languages"` → `syntax_languages`; mirrors the schema module.
fn slug(label: &str) -> String {
    label
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn lookup(key: &str) -> Option<String> {
    fresh_i18n::translate_in(LOCALE, key).map(str::to_string)
}

fn field_key(path: &str) -> String {
    format!("settings.field{}", path.replace('/', "."))
}

/// How many labels the catalog translated, so the test cannot pass by
/// never finding a key.
#[derive(Default)]
struct Translated {
    names: usize,
    descriptions: usize,
    categories: usize,
    sections: usize,
}

/// Check one setting and everything nested in it. `key_path` is where the
/// setting sits in the config: a nested object's fields carry paths relative
/// to the object, and map values and array items stand for any key, `*`.
fn check_setting(en: &SettingSchema, ja: &SettingSchema, key_path: &str, count: &mut Translated) {
    assert_eq!(en.path, ja.path);

    if let Some(name) = lookup(&field_key(key_path)) {
        assert_eq!(ja.name, name, "name of {}", key_path);
        count.names += 1;
    }

    match lookup(&format!("{}_desc", field_key(key_path))) {
        Some(desc) => {
            assert_eq!(ja.description.as_deref(), Some(desc.as_str()), "description of {}", key_path);
            count.descriptions += 1;
        }
        None => assert_eq!(ja.description, en.description, "untranslated description of {}", key_path),
    }

    // A section stays untranslated in the schema, since pages group and order
    // by it; it is translated where it is shown. The locale is Japanese by
    // the time this runs.
    assert_eq!(ja.section, en.section, "section of {}", en.path);
    if let Some(sec) = &en.section {
        match lookup(&format!("settings.section.{}", slug(sec))) {
            Some(label) => {
                assert_eq!(section_display_name(sec), label, "section {}", sec);
                count.sections += 1;
            }
            None => assert_eq!(&section_display_name(sec), sec, "untranslated section {}", sec),
        }
    }

    match (&en.setting_type, &ja.setting_type) {
        (SettingType::Object { properties: en_p }, SettingType::Object { properties: ja_p }) => {
            assert_eq!(en_p.len(), ja_p.len(), "fields of {}", key_path);
            for (en_c, ja_c) in en_p.iter().zip(ja_p) {
                check_setting(en_c, ja_c, &format!("{}{}", key_path, en_c.path), count);
            }
        }
        (SettingType::Map { value_schema: en_v, .. }, SettingType::Map { value_schema: ja_v, .. })
        | (
            SettingType::ObjectArray { item_schema: en_v, .. },
            SettingType::ObjectArray { item_schema: ja_v, .. },
        ) => check_setting(en_v, ja_v, &format!("{}/*", key_path), count),
        _ => {}
    }
}

fn check_category(en: &SettingCategory, ja: &SettingCategory, count: &mut Translated) {
    // The internal name keys the tree's nesting, order and icons, so it must
    // not change with the locale.
    assert_eq!(en.name, ja.name);
    assert_eq!(en.display_name, en.name, "English display name of {}", en.name);

    match lookup(&format!("settings.category.{}", slug(&en.name))) {
        Some(name) => {
            assert_eq!(ja.display_name, name, "display name of {}", en.name);
            count.categories += 1;
        }
        None => assert_eq!(ja.display_name, en.name, "untranslated display name of {}", en.name),
    }
    match lookup(&format!("settings.category.{}_desc", slug(&en.name))) {
        Some(desc) => assert_eq!(ja.description.as_deref(), Some(desc.as_str())),
        None => assert_eq!(ja.description, en.description),
    }

    // Settings are ordered by path, so the order is the same in every locale.
    let paths = |c: &SettingCategory| c.settings.iter().map(|s| s.path.clone()).collect::<Vec<_>>();
    assert_eq!(paths(en), paths(ja), "settings in {}", en.name);
    for (en_s, ja_s) in en.settings.iter().zip(&ja.settings) {
        check_setting(en_s, ja_s, &en_s.path, count);
    }
    assert_eq!(en.subcategories.len(), ja.subcategories.len());
    for (en_c, ja_c) in en.subcategories.iter().zip(&ja.subcategories) {
        check_category(en_c, ja_c, count);
    }
}

#[test]
fn test_settings_schema_labels_follow_the_locale() {
    // Switches the process-wide locale; see `global_state`.
    let _pin = crate::common::global_state::pin_config_globals();
    fresh::i18n::init_with_config(Some("en"));
    let en = parse_schema(SCHEMA_JSON).unwrap();
    fresh::i18n::init_with_config(Some(LOCALE));
    let ja = parse_schema(SCHEMA_JSON).unwrap();

    assert_eq!(en.len(), ja.len());
    let mut count = Translated::default();
    for (en_c, ja_c) in en.iter().zip(&ja) {
        check_category(en_c, ja_c, &mut count);
    }

    assert!(count.names > 0, "no setting name was translated");
    assert!(count.descriptions > 0, "no setting description was translated");
    assert!(count.categories > 0, "no category name was translated");
    assert!(count.sections > 0, "no section was translated");
}
