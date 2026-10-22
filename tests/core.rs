use cliprust::{
    model::{Bundle, MAX_BUNDLE_BYTES, confidential},
    persistence::{Archive, Filter, Store},
    search::{self, Weights},
    snippets::{self, Snippet},
};
use std::collections::BTreeMap;

#[test]
fn unified_snippet_search_ranks_before_limiting_and_keeps_action_ids() {
    let mut templates: Vec<_> = (0..350)
        .map(|i| Snippet {
            name: format!("template {i}"),
            body: "unrelated".into(),
            tags: vec![],
            abbreviation: String::new(),
            pinned: false,
        })
        .collect();
    templates[348].body = format!("{} rareending", "x".repeat(9000));
    assert!(snippets::matches(&templates[348], "rareending"));
    templates[349].name = "Rust Systems Guide".into();
    templates[349].abbreviation = "rsg".into();
    templates[349].pinned = true;
    let mut db = Store::memory().unwrap();
    db.capture(&Bundle::text("r distant s distant g"), None, 10)
        .unwrap();
    let history = db.list("rsg", &Filter::default(), 10, 300).unwrap();
    let ranked = snippets::rank_with_history(
        history,
        &templates,
        "rsg",
        10,
        &Weights::default(),
        false,
        1,
    );
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].id, -350);
    assert_eq!(ranked[0].preview, "Rust Systems Guide");
    assert!(snippets::matches(&templates[349], "rst sys"));
    let favorites =
        snippets::rank_with_history(vec![], &templates, "", 10, &Weights::default(), true, 300);
    assert_eq!(favorites.len(), 1);
    assert_eq!(favorites[0].id, -350);
}

#[test]
fn explicit_duplicates_are_independent_and_survive_archive_roundtrip() {
    let mut db = Store::memory().unwrap();
    let bundle = Bundle::text("editable reference");
    let original = db.capture(&bundle, Some("Editor"), 10).unwrap();
    let duplicate = db.duplicate(original, 20).unwrap();
    assert_ne!(original, duplicate);
    assert_eq!(db.bundle(duplicate).unwrap().formats, bundle.formats);
    assert_eq!(db.capture(&bundle, None, 30).unwrap(), original);
    db.pin(duplicate).unwrap();
    let mut restored = Store::memory().unwrap();
    restored.import(db.export().unwrap()).unwrap();
    restored.import(db.export().unwrap()).unwrap();
    let entries = restored.list("", &Filter::default(), 40, 10).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries.iter().filter(|entry| entry.pinned).count(), 1);
    db.delete(original).unwrap();
    assert_eq!(db.bundle(duplicate).unwrap().formats, bundle.formats);
    assert!(db.duplicate(original, 50).is_err());
}

#[test]
fn grouping_uses_local_dates_and_preserves_order_within_sections() {
    use chrono::TimeZone;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
    let timestamp = |day| {
        chrono::Local
            .with_ymd_and_hms(2026, 10, day, 12, 0, 0)
            .unwrap()
            .timestamp()
    };
    let mut db = Store::memory().unwrap();
    for (text, day, source) in [
        ("today one", 22, Some("Editor")),
        ("today two", 22, Some("Browser")),
        ("yesterday", 21, None),
        ("older", 19, None),
        ("pinned", 18, Some("Editor")),
    ] {
        let id = db
            .capture(&Bundle::text(text), source, timestamp(day))
            .unwrap();
        if text == "pinned" {
            db.pin(id).unwrap();
        }
    }
    let mut entries = db.list("", &Filter::default(), timestamp(22), 20).unwrap();
    let original: Vec<_> = entries.iter().map(|entry| entry.id).collect();
    assert!(
        cliprust::grouping::group(&mut entries, "none", today)
            .iter()
            .all(String::is_empty)
    );
    assert_eq!(
        entries.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        original
    );
    let today_ids: Vec<_> = entries
        .iter()
        .filter(|e| e.last_used == timestamp(22))
        .map(|e| e.id)
        .collect();
    let labels = cliprust::grouping::group(&mut entries, "date", today);
    assert_eq!(
        labels,
        ["Pinned", "Today", "", "Yesterday", "Previous days"]
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.last_used == timestamp(22))
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        today_ids
    );
    let labels = cliprust::grouping::group(&mut entries, "application", today);
    assert_eq!(labels, ["Browser", "Editor", "", "Unknown application", ""]);
    let labels = cliprust::grouping::group(&mut entries, "type", today);
    assert_eq!(labels, ["Text", "", "", "", ""]);
    let labels = cliprust::grouping::group(&mut entries, "pins", today);
    assert_eq!(labels, ["Pinned", "Unpinned", "", "", ""]);
}

#[test]
fn backup_restore_merges_history_without_changing_settings_or_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("backup.db");
    let mut original = Store::memory().unwrap();
    let id = original
        .capture(&Bundle::text("backed up"), Some("editor"), 20)
        .unwrap();
    original.pin(id).unwrap();
    let mut snippet = Snippet {
        name: "same name".into(),
        body: "old body".into(),
        tags: vec![],
        abbreviation: String::new(),
        pinned: false,
    };
    original.save_snippet(&snippet).unwrap();
    original.set_setting("theme", "light").unwrap();
    original.backup(&path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut current = Store::memory().unwrap();
    current
        .capture(&Bundle::text("newer local"), None, 30)
        .unwrap();
    current.set_setting("theme", "dark").unwrap();
    snippet.body = "new local body".into();
    current.save_snippet(&snippet).unwrap();
    current.merge_backup(&path).unwrap();
    assert_eq!(
        current.list("", &Filter::default(), 40, 20).unwrap().len(),
        2
    );
    assert_eq!(
        current
            .list(
                "",
                &Filter {
                    pinned: true,
                    ..Filter::default()
                },
                40,
                20
            )
            .unwrap()
            .len(),
        1
    );
    assert_eq!(current.setting("theme").as_deref(), Some("dark"));
    assert_eq!(current.snippets().unwrap()[0].body, "new local body");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let invalid = dir.path().join("invalid.db");
    std::fs::write(&invalid, "invalid backup").unwrap();
    assert!(current.merge_backup(&invalid).is_err());
    assert_eq!(
        current.list("", &Filter::default(), 40, 20).unwrap().len(),
        2
    );
}

#[test]
fn configurable_shortcuts_reject_collisions_and_preserve_typing() {
    use cliprust::shortcuts::{Shortcut, parse_keymap};
    let map = parse_keymap("{}").unwrap();
    assert_ne!(map["paste"], map["keep-open"]);
    assert_eq!(
        Shortcut::global("alt+shift+v").unwrap().portal_trigger(),
        "ALT+SHIFT+V"
    );
    assert_eq!(
        Shortcut::global("Meta+V").unwrap(),
        Shortcut::global("Super+V").unwrap()
    );
    assert_eq!(
        Shortcut::global("Mod+V").unwrap(),
        Shortcut::global("Super+V").unwrap()
    );
    assert_eq!(
        Shortcut::parse("Control+Shift+Enter").unwrap(),
        Shortcut::parse("Ctrl+Shift+Enter").unwrap()
    );
    assert_eq!(map["keep-open"], Shortcut::parse("Ctrl+Enter").unwrap());
    assert!(Shortcut::global("Enter").is_err());
    for value in [
        "A",
        "Shift+A",
        "Ctrl+Ctrl+A",
        "Ctrl+",
        "Ctrl+Left",
        "Ctrl+A+B",
    ] {
        assert!(Shortcut::parse(value).is_err(), "{value}");
    }
    for value in [
        r#"{"paste":"Ctrl+C"}"#,
        r#"{"paste":"Ctrl+A"}"#,
        r#"{"unknown":"Ctrl+R"}"#,
    ] {
        assert!(parse_keymap(value).is_err());
    }
    let custom = parse_keymap(r#"{"paste":"Alt+Enter","keep-open":"Ctrl+Shift+Enter"}"#).unwrap();
    assert!(custom["paste"].alt);
    assert!(custom["keep-open"].shift);
    assert_eq!(custom["copy"], map["copy"]);
}

#[test]
fn recent_searches_are_bounded_deduplicated_persistent_and_pause_aware() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("search.db");
    let db = Store::open(&path).unwrap();
    for i in 0..25 {
        db.remember_search(&format!("query {i}")).unwrap();
    }
    db.remember_search(" query 20 ").unwrap();
    let recent = db.recent_searches();
    assert_eq!(recent.len(), 20);
    assert_eq!(recent[0], "query 20");
    assert_eq!(recent.iter().filter(|q| *q == "query 20").count(), 1);
    db.set_setting("paused", "true").unwrap();
    db.remember_search("private query").unwrap();
    assert_eq!(db.recent_searches(), recent);
    drop(db);
    let db = Store::open(&path).unwrap();
    assert_eq!(db.recent_searches(), recent);
    db.set_setting("recent_searches", "[]").unwrap();
    assert!(db.recent_searches().is_empty());
}

#[test]
fn capture_exclusions_and_pause_are_shared_and_fail_closed() {
    let db = Store::memory().unwrap();
    assert!(db.capture_allowed(Some("keepassxc")));
    db.set_setting("excluded_apps", r#"["KeePassXC","secret.exe"]"#)
        .unwrap();
    assert!(!db.capture_allowed(Some("keepassxc")));
    assert!(!db.capture_allowed(Some("SECRET.EXE")));
    assert!(db.capture_allowed(Some("keepassxc-helper")));
    assert!(db.capture_allowed(None));
    db.set_setting("paused", "true").unwrap();
    assert!(!db.capture_allowed(None));
    db.set_setting("paused", "false").unwrap();
    db.set_setting("excluded_apps", "invalid").unwrap();
    assert!(!db.capture_allowed(Some("editor")));
    assert!(!db.capture_allowed(None));
    for invalid in [r#"[""]"#, r#"[" app"]"#, r#"["app\nname"]"#, "{}"] {
        assert!(cliprust::security::parse_exclusions(invalid).is_err());
    }
}

#[test]
fn filters_are_applied_before_bounded_search_candidates() {
    let mut db = Store::memory().unwrap();
    for index in 0..1100 {
        db.capture(
            &Bundle::text(&format!("needle distractor {index}")),
            Some("browser"),
            1000 + index,
        )
        .unwrap();
    }
    let target = db
        .capture(&Bundle::text("needle target"), Some("editor"), 10)
        .unwrap();
    for filter in [
        Filter {
            source: Some("editor".into()),
            ..Filter::default()
        },
        Filter {
            since: Some(10),
            until: Some(11),
            ..Filter::default()
        },
    ] {
        for query in ["", "needle"] {
            let matches = db.list(query, &filter, 3000, 10).unwrap();
            assert_eq!(
                matches.iter().map(|entry| entry.id).collect::<Vec<_>>(),
                [target]
            );
        }
    }
}

#[test]
fn bundles_preserve_formats_and_deduplicate_only_full_identity() {
    let mut db = Store::memory().unwrap();
    let plain = Bundle::text("hello 🦀 مرحبا");
    let mut rich = plain.clone();
    rich.formats
        .insert("text/html".into(), b"<b>hello</b>".to_vec());
    let a = db.capture(&plain, None, 100).unwrap();
    let b = db.capture(&plain, None, 101).unwrap();
    let c = db.capture(&rich, None, 102).unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(db.bundle(c).unwrap(), rich);
    assert_eq!(
        db.list("", &Filter::default(), 102, 10)
            .unwrap()
            .iter()
            .find(|e| e.id == a)
            .unwrap()
            .copy_count,
        2
    );
}
#[test]
fn persistence_migration_fts_pins_and_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.db");
    let mut db = Store::open(&path).unwrap();
    let pin = db
        .capture(&Bundle::text("permanent engineering note"), None, 1)
        .unwrap();
    db.pin(pin).unwrap();
    db.capture(&Bundle::text("old temporary note"), None, 2)
        .unwrap();
    let newest = db
        .capture(
            &Bundle::text("SQLite transaction isolation"),
            Some("editor"),
            900000,
        )
        .unwrap();
    assert_eq!(
        db.list("transact", &Filter::default(), 900001, 10).unwrap()[0].id,
        newest
    );
    db.cleanup(1, 1, 900001).unwrap();
    drop(db);
    let db = Store::open(&path).unwrap();
    assert_eq!(db.integrity().unwrap(), "ok");
    assert_eq!(
        db.bundle(pin).unwrap().plain_text().unwrap(),
        "permanent engineering note"
    );
    assert_eq!(
        db.list("", &Filter::default(), 900001, 10).unwrap().len(),
        2
    );
    db.clear_unpinned().unwrap();
    assert_eq!(
        db.list("", &Filter::default(), 900001, 10).unwrap().len(),
        1
    );
}
#[test]
fn ranking_is_unicode_aware_deterministic_and_handles_typos() {
    let mut db = Store::memory().unwrap();
    let exact = db.capture(&Bundle::text("café"), None, 1).unwrap();
    db.capture(&Bundle::text("a café near the office"), None, 2)
        .unwrap();
    db.capture(&Bundle::text("clipboard history manager"), None, 3)
        .unwrap();
    db.capture(&Bundle::text("irrelevant"), None, 4).unwrap();
    assert_eq!(search::normalize("ＣＡＦÉ"), "café");
    let results = db.list("cafe\u{301}", &Filter::default(), 10, 10).unwrap();
    assert_eq!(results[0].id, exact);
    assert_eq!(
        db.list("chm", &Filter::default(), 10, 10).unwrap()[0].preview,
        "clipboard history manager"
    );
    assert_eq!(
        db.list("clpbord", &Filter::default(), 10, 10)
            .unwrap()
            .len(),
        1
    );
    let a = db.list("café", &Filter::default(), 10, 10).unwrap();
    let b = db.list("café", &Filter::default(), 10, 10).unwrap();
    assert_eq!(
        a.iter().map(|e| e.id).collect::<Vec<_>>(),
        b.iter().map(|e| e.id).collect::<Vec<_>>()
    );
    assert!(search::score(&a[0], "totally absent", 10, &Weights::default()).is_none());
}

#[test]
fn pinned_items_can_be_reordered_and_unpinned_items_are_ignored() {
    let mut db = Store::memory().unwrap();
    let first = db.capture(&Bundle::text("first pin"), None, 1).unwrap();
    let second = db.capture(&Bundle::text("second pin"), None, 2).unwrap();
    db.pin(first).unwrap();
    db.pin(second).unwrap();
    assert!(!db.move_pin(first, -1).unwrap());
    assert!(db.move_pin(second, -1).unwrap());
    let pins = db
        .list(
            "",
            &Filter {
                pinned: true,
                ..Filter::default()
            },
            3,
            10,
        )
        .unwrap();
    assert_eq!(
        pins.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        [second, first]
    );
    db.pin(second).unwrap();
    assert!(!db.move_pin(second, 1).unwrap());
}

#[test]
fn date_filter_uses_an_inclusive_start_and_exclusive_end() {
    let mut db = Store::memory().unwrap();
    let before = db.capture(&Bundle::text("before range"), None, 99).unwrap();
    let start = db
        .capture(&Bundle::text("at range start"), None, 100)
        .unwrap();
    db.capture(&Bundle::text("inside range"), None, 199)
        .unwrap();
    let end = db
        .capture(&Bundle::text("at range end"), None, 200)
        .unwrap();
    let found = db
        .list(
            "",
            &Filter {
                since: Some(100),
                until: Some(200),
                ..Filter::default()
            },
            201,
            10,
        )
        .unwrap();
    let ids = found.iter().map(|entry| entry.id).collect::<Vec<_>>();
    assert!(!ids.contains(&before));
    assert!(ids.contains(&start));
    assert!(!ids.contains(&end));
    assert_eq!(found.len(), 2);
}
#[test]
fn roundtrip_archive_backup_and_import_rollback() {
    let mut db = Store::memory().unwrap();
    let id = db.capture(&Bundle::text("archive"), None, 123).unwrap();
    db.pin(id).unwrap();
    db.save_snippet(&Snippet {
        name: "Greeting".into(),
        body: "Hi {{date}}".into(),
        tags: vec!["work".into()],
        abbreviation: "!hi".into(),
        pinned: true,
    })
    .unwrap();
    let json = serde_json::to_vec(&db.export().unwrap()).unwrap();
    let archive: Archive = serde_json::from_slice(&json).unwrap();
    let mut restored = Store::memory().unwrap();
    restored.import(archive).unwrap();
    assert!(restored.snippets().unwrap()[0].pinned);
    assert!(restored.list("", &Filter::default(), 200, 10).unwrap()[0].pinned);
    let mut broken = db.export().unwrap();
    broken.entries[0].1 = Bundle::text("new entry");
    broken.snippets[0].name = "".into();
    assert!(restored.import(broken).is_err());
    assert_eq!(
        restored
            .list("", &Filter::default(), 200, 10)
            .unwrap()
            .len(),
        1
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("backup.db");
    db.backup(&path).unwrap();
    assert_eq!(Store::open(&path).unwrap().integrity().unwrap(), "ok");
}
#[test]
fn malformed_binary_and_oversized_content_are_safe() {
    let binary = Bundle {
        formats: BTreeMap::from([("text/plain".into(), vec![0xff, 0xfe])]),
    };
    binary.validate().unwrap();
    assert_eq!(binary.plain_text(), None);
    let oversized = Bundle {
        formats: BTreeMap::from([("image/png".into(), vec![0; MAX_BUNDLE_BYTES + 1])]),
    };
    assert!(oversized.validate().is_err());
    assert!(
        Bundle {
            formats: BTreeMap::new()
        }
        .validate()
        .is_err()
    );
    assert!(confidential(&[
        "text/plain".into(),
        "x-kde-passwordManagerHint".into()
    ]));
}
#[test]
fn image_and_file_bundles_survive_storage_without_reading_file_contents() {
    let mut db = Store::memory().unwrap();
    for bundle in [
        Bundle {
            formats: BTreeMap::from([("image/png".into(), vec![137, 80, 78, 71, 1, 2, 3])]),
        },
        Bundle {
            formats: BTreeMap::from([(
                "text/uri-list".into(),
                b"file:///tmp/a%20b.txt\r\nfile:///tmp/c.txt\r\n".to_vec(),
            )]),
        },
    ] {
        let id = db.capture(&bundle, None, 1).unwrap();
        assert_eq!(db.bundle(id).unwrap(), bundle);
    }
}

#[test]
fn file_reference_bundle_converts_uri_list_to_native_paths() {
    let path = std::env::temp_dir().join("Cliprust shared file.txt");
    let file_url = url::Url::from_file_path(&path).unwrap();
    let bundle = Bundle {
        formats: BTreeMap::from([(
            "text/uri-list".into(),
            format!("# copied file\r\n{}\r\n", file_url).into_bytes(),
        )]),
    };
    assert_eq!(
        bundle.file_paths().unwrap().unwrap(),
        [path.to_string_lossy().into_owned()]
    );
}
#[test]
fn snippet_expansion_is_single_pass_and_unknown_placeholders_survive() {
    let now = chrono::Local::now();
    assert_eq!(
        snippets::expand("{{clipboard}} / {{unknown}}", now, "{{date}}"),
        "{{date}} / {{unknown}}"
    );
    assert!(
        snippets::expand("{{date}} {{time}}", now, "")
            .starts_with(&now.format("%Y-%m-%d").to_string())
    );
}
#[test]
fn snippet_search_matches_name_body_tags_and_alias_case_insensitively() {
    let snippet = Snippet {
        name: "Support reply".into(),
        body: "Hello, {{clipboard}}".into(),
        tags: vec!["customer care".into()],
        abbreviation: ";sup".into(),
        pinned: false,
    };
    for query in ["SUPPORT", "clipboard", "CUSTOMER", ";SUP"] {
        assert!(snippets::matches(&snippet, query));
    }
    assert!(!snippets::matches(&snippet, "billing"));
}
#[test]
fn snippets_default_to_unpinned_when_loading_old_json() {
    let old = r#"{"name":"Old","body":"text","tags":[],"abbreviation":";old"}"#;
    let snippet: Snippet = serde_json::from_str(old).unwrap();
    assert!(!snippet.pinned);
}
#[test]
fn wayland_paste_never_claims_ungranted_capability() {
    #[cfg(target_os = "linux")]
    if cliprust::platform::sway::available() || cliprust::platform::kde::is_kde() {
        return; // Desktop integrations validate their own focus/input capability.
    }
    if cliprust::platform::is_wayland() {
        assert!(
            cliprust::platform::can_paste(cliprust::platform::Target(42))
                .unwrap_err()
                .to_string()
                .contains("Wayland")
        );
    }
}
#[test]
fn filters_and_delete_keep_fts_consistent() {
    let mut db = Store::memory().unwrap();
    let id = db
        .capture(&Bundle::text("https://example.com"), Some("browser"), 4)
        .unwrap();
    db.capture(&Bundle::text("#ffeeaa"), Some("editor"), 5)
        .unwrap();
    assert_eq!(db.sources().unwrap(), ["browser", "editor"]);
    assert_eq!(
        db.list(
            "",
            &Filter {
                kind: Some("link".into()),
                source: Some("browser".into()),
                since: Some(3),
                ..Filter::default()
            },
            6,
            10
        )
        .unwrap()
        .len(),
        1
    );
    db.delete(id).unwrap();
    assert!(
        db.list("example", &Filter::default(), 6, 10)
            .unwrap()
            .is_empty()
    );
}
#[test]
#[ignore = "explicit performance measurement: cargo test --release --test core benchmark_10000 -- --ignored --nocapture"]
fn benchmark_10000() {
    let directory = tempfile::tempdir().unwrap();
    for (label, mut db) in [
        ("memory", Store::memory().unwrap()),
        (
            "disk WAL",
            Store::open(&directory.path().join("benchmark.db")).unwrap(),
        ),
    ] {
        let start = std::time::Instant::now();
        for i in 0..10_000 {
            db.capture(
                &Bundle::text(&format!(
                    "project {i} engineering clipboard transaction reference"
                )),
                None,
                i,
            )
            .unwrap();
        }
        println!("{label}: 10000 inserts {:?}", start.elapsed());
        for query in ["transaction", "project 9876", "engneerng"] {
            let _ = db.list(query, &Filter::default(), 10001, 100).unwrap();
            let mut samples = Vec::new();
            for _ in 0..20 {
                let start = std::time::Instant::now();
                assert!(
                    !db.list(query, &Filter::default(), 10001, 100)
                        .unwrap()
                        .is_empty()
                );
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "{label}: query {query:?}, n=20 median={:.3}ms p95={:.3}ms max={:.3}ms",
                samples[10], samples[18], samples[19]
            );
        }
    }
}

#[test]
fn retention_settings_persist_and_weights_are_configurable() {
    let db = Store::memory().unwrap();
    assert_eq!(db.retention(), (10000, 30));
    db.set_setting("max_entries", "500").unwrap();
    db.set_setting("retention_days", "0").unwrap();
    assert_eq!(db.retention(), (500, 0));
    let weights = Weights {
        pinned: 2000.,
        ..Weights::default()
    };
    db.set_setting("search_weights", &serde_json::to_string(&weights).unwrap())
        .unwrap();
    let restored: Weights = serde_json::from_str(&db.setting("search_weights").unwrap()).unwrap();
    assert_eq!(restored.pinned, 2000.);
}

#[test]
fn pause_invalidates_inflight_capture_even_after_resuming() {
    use cliprust::platform::{capture_token, set_capture_paused, token_current};
    set_capture_paused(false);
    let old = capture_token().unwrap();
    set_capture_paused(true);
    assert!(capture_token().is_none());
    assert!(!token_current(old));
    set_capture_paused(false);
    assert!(!token_current(old));
    assert!(token_current(capture_token().unwrap()));
}

#[test]
fn untrusted_archive_timestamps_do_not_overflow_ranking() {
    let mut db = Store::memory().unwrap();
    db.capture(&Bundle::text("extreme timestamp"), None, 1)
        .unwrap();
    let mut archive = db.export().unwrap();
    archive.entries[0].0.last_used = i64::MIN;
    let mut restored = Store::memory().unwrap();
    restored.import(archive).unwrap();
    assert_eq!(
        restored
            .list("timestamp", &Filter::default(), 100, 10)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn open_history_snapshot_freezes_order_search_metadata_and_snippet_identity() {
    use cliprust::history_session::{EntryKey, HistorySession};
    let mut db = Store::memory().unwrap();
    let first = db.capture(&Bundle::text("session alpha"), None, 1).unwrap();
    let pinned = db
        .capture(&Bundle::text("session pinned"), None, 2)
        .unwrap();
    db.pin(pinned).unwrap();
    let latest = db
        .capture(&Bundle::text("session newest مرحبا"), None, 3)
        .unwrap();
    db.save_snippet(&Snippet {
        name: "Z template".into(),
        body: "session body".into(),
        tags: vec![],
        abbreviation: "".into(),
        pinned: false,
    })
    .unwrap();
    let session = HistorySession::capture(&db).unwrap();
    let ids =
        |entries: Vec<cliprust::model::Entry>| entries.iter().map(EntryKey::of).collect::<Vec<_>>();
    let before = ids(session.list("", &Filter::default(), false));
    let search_before = ids(session.list("session", &Filter::default(), false));
    assert_eq!(before[0], EntryKey::History(pinned));
    assert_eq!(session.latest, Some(latest));
    db.used(first, 100).unwrap();
    let outside = db
        .capture(&Bundle::text("session outside"), None, 101)
        .unwrap();
    db.save_snippet(&Snippet {
        name: "A template".into(),
        body: "session new template".into(),
        tags: vec![],
        abbreviation: "".into(),
        pinned: false,
    })
    .unwrap();
    assert_eq!(ids(session.list("", &Filter::default(), false)), before);
    assert_eq!(
        ids(session.list("session", &Filter::default(), false)),
        search_before
    );
    let reopened = HistorySession::capture(&db).unwrap();
    assert_eq!(reopened.latest, Some(outside));
    assert_eq!(reopened.list("", &Filter::default(), false)[1].id, outside);
    db.used(first, 102).unwrap();
    assert_eq!(HistorySession::capture(&db).unwrap().latest, Some(outside));
    // Genuine external copying of identical content is still a capture, not a duplicate.
    assert_eq!(
        db.capture(&Bundle::text("session alpha"), None, 103)
            .unwrap(),
        first
    );
    assert_eq!(HistorySession::capture(&db).unwrap().latest, Some(first));
    assert_eq!(db.history_snapshot().unwrap().len(), 4);
    db.delete(first).unwrap();
    assert!(HistorySession::capture(&db).unwrap().latest.is_some());
}

#[test]
fn empty_session_and_filters_have_no_phantom_selection_candidates() {
    use cliprust::history_session::HistorySession;
    let mut db = Store::memory().unwrap();
    let empty = HistorySession::capture(&db).unwrap();
    assert!(empty.latest.is_none());
    assert!(empty.list("", &Filter::default(), false).is_empty());
    db.capture(&Bundle::text("hello"), Some("Editor"), 1)
        .unwrap();
    let session = HistorySession::capture(&db).unwrap();
    assert!(
        session
            .list(
                "",
                &Filter {
                    source: Some("Browser".into()),
                    ..Default::default()
                },
                false
            )
            .is_empty()
    );
}

#[test]
fn latest_copy_remains_selectable_when_pins_exceed_the_visible_limit() {
    use cliprust::history_session::HistorySession;
    let mut db = Store::memory().unwrap();
    for i in 0..305 {
        let id = db
            .capture(&Bundle::text(&format!("pin {i}")), None, i)
            .unwrap();
        db.pin(id).unwrap();
    }
    let latest = db.capture(&Bundle::text("latest"), None, 400).unwrap();
    let entries = HistorySession::capture(&db)
        .unwrap()
        .list("", &Filter::default(), false);
    assert_eq!(entries.len(), 300);
    assert_eq!(entries.last().unwrap().id, latest);
    assert!(entries[..299].iter().all(|e| e.pinned));
}
