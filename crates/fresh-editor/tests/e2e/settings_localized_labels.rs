//! E2E: Settings shows the config schema's labels in the configured locale —
//! category names, section headers, setting names and descriptions — and
//! keeps the page layout the same as in English.

use crate::common::global_state::pin_config_globals;
use crate::common::harness::EditorTestHarness;
use crossterm::event::{KeyCode, KeyModifiers};
use fresh::config::Config;
use unicode_width::UnicodeWidthChar;

/// Settings opened in Japanese.
///
/// Through the palette by the command's Japanese name: the harness's own
/// `open_settings` types the English one, which a Japanese palette does not
/// list.
fn japanese_settings() -> EditorTestHarness {
    let config = Config {
        locale: Some("ja").into(),
        ..Default::default()
    };
    let mut harness = EditorTestHarness::with_config(120, 40, config).unwrap();
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.wait_for_prompt().unwrap();
    harness.type_text("設定を開く").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.wait_for_screen_contains("Settings [").unwrap();
    harness
}

/// The screen as it reads. A wide glyph covers two cells and the screen
/// string has a symbol for each, so the second — the glyph's continuation
/// cell — is dropped: `一 般` reads `一般`.
fn read(harness: &EditorTestHarness) -> String {
    let mut out = String::new();
    for line in harness.screen_to_string().lines() {
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            out.push(c);
            if c.width() == Some(2) {
                chars.next();
            }
        }
        out.push('\n');
    }
    out
}

/// Walk the category tree down until the row naming `category` carries the
/// selection marker.
fn select_category(harness: &mut EditorTestHarness, category: &str) {
    let selected = |screen: &str| {
        screen
            .lines()
            .flat_map(|l| l.split('│'))
            .any(|cell| cell.trim_start().starts_with('>') && cell.contains(category))
    };
    for _ in 0..30 {
        if selected(&read(harness)) {
            return;
        }
        harness.send_key(KeyCode::Down, KeyModifiers::NONE).unwrap();
        harness.render().unwrap();
    }
    panic!("{category:?} never became selected:\n{}", read(harness));
}

/// The labels of the category tree, top to bottom, from the expanded
/// `category` row down.
///
/// The tree is one `│`-separated column of the dialog: the one holding that
/// row. The same words also appear in the menu bar and on the page beside the
/// tree, so only that column is read.
fn tree_labels(screen: &str, category: &str) -> Vec<String> {
    let lines: Vec<&str> = screen.lines().collect();
    let (top, column) = lines
        .iter()
        .enumerate()
        .find_map(|(row, l)| {
            l.split('│')
                .position(|cell| cell.contains('▼') && cell.contains(category))
                .map(|column| (row, column))
        })
        .unwrap_or_else(|| panic!("{category:?} is not expanded in the tree:\n{screen}"));
    lines[top..]
        .iter()
        .filter_map(|l| l.split('│').nth(column))
        .map(|cell| cell.trim().to_string())
        .collect()
}

#[test]
fn settings_labels_follow_the_locale() {
    // Pins Japanese for the whole body; see `global_state`.
    let _pin = pin_config_globals();
    let harness = japanese_settings();
    let screen = read(&harness);

    for label in [
        // The category tree, by the names the catalog gives them.
        "一般",
        "エディタ",
        // The first page is General: a setting's name and its description.
        "テーマ",
        "カラーテーマの名前",
    ] {
        assert!(
            screen.contains(label),
            "{label:?} is not on screen:\n{screen}"
        );
    }
    // Nothing of the English schema is left on it.
    assert!(!screen.contains("Color theme name"), "{screen}");
}

/// Pages are laid out by the schema, not by the translated labels: sections
/// keep the order they have in English. Sorted by their Japanese names they
/// would come out in code-point order — `LSP` ahead of `括弧の対応`.
#[test]
fn sections_keep_their_english_order() {
    let _pin = pin_config_globals();
    let mut harness = japanese_settings();

    select_category(&mut harness, "エディタ");
    // Expanding the category lists its sections under it in the tree.
    harness
        .send_key(KeyCode::Right, KeyModifiers::NONE)
        .unwrap();
    harness
        .wait_until(|h| read(h).contains("空白文字"))
        .unwrap();

    let screen = read(&harness);
    let labels = tree_labels(&screen, "エディタ");
    // "Bracket Matching" < "Completion" < "Keyboard" < "LSP" < "Whitespace".
    let order = ["括弧の対応", "補完", "キーボード", "LSP", "空白文字"];
    let rows: Vec<usize> = order
        .iter()
        .map(|s| {
            labels
                .iter()
                .position(|l| l == s)
                .unwrap_or_else(|| panic!("section {s:?} is not in the tree:\n{screen}"))
        })
        .collect();
    assert!(
        rows.windows(2).all(|w| w[0] < w[1]),
        "sections out of order {rows:?}:\n{screen}"
    );

    // The page beside the tree heads each section's cards with the same
    // translated label — a separate path from the tree's.
    let page = page_labels(&screen, "エディタ");
    assert!(
        page.iter().any(|l| l == "括弧の対応"),
        "the page has no translated section heading:\n{screen}"
    );
    assert!(
        !page.iter().any(|l| l == "Bracket Matching"),
        "the page still heads a section in English:\n{screen}"
    );
}

/// The column of the dialog right of the tree: the selected page. Each cell
/// is trimmed, so a section heading — a row of its own — reads as its label.
fn page_labels(screen: &str, category: &str) -> Vec<String> {
    let column = screen
        .lines()
        .find_map(|l| {
            l.split('│')
                .position(|cell| cell.contains('▼') && cell.contains(category))
        })
        .unwrap_or_else(|| panic!("{category:?} is not expanded in the tree:\n{screen}"))
        + 1;
    screen
        .lines()
        .filter_map(|l| l.split('│').nth(column))
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// An entry dialog heads its sections too, on a path of its own: the fields
/// of an LSP server's Edit Item dialog end in an `Advanced` section.
#[test]
fn entry_dialog_sections_follow_the_locale() {
    let _pin = pin_config_globals();
    let mut harness = japanese_settings();

    // Search reaches the LSP map by its path, whatever the locale.
    harness
        .send_key(KeyCode::Char('/'), KeyModifiers::NONE)
        .unwrap();
    harness.type_text("lsp").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();
    // Down to the python row, whose edit hint shows once it is focused.
    let focused = |h: &EditorTestHarness| {
        read(h)
            .lines()
            .any(|l| l.contains("python") && l.contains("[Enter to edit]"))
    };
    for _ in 0..80 {
        if focused(&harness) {
            break;
        }
        harness.send_key(KeyCode::Down, KeyModifiers::NONE).unwrap();
        harness.render().unwrap();
    }
    assert!(
        focused(&harness),
        "python never focused:\n{}",
        read(&harness)
    );

    // Edit Value for python, then Edit Item for its first server. Their
    // titles carry the translated field name, so each is recognised by
    // something the locale does not touch: the first by its frame, the
    // second by the server's command shown as an editable value.
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.wait_until(|h| read(h).contains("╭ Edit")).unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.wait_until(|h| read(h).contains("[pylsp")).unwrap();

    let screen = read(&harness);
    assert!(screen.contains("── 詳細 ──"), "{screen}");
    assert!(!screen.contains("── Advanced ──"), "{screen}");
}
