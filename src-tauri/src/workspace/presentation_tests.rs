#![expect(
    clippy::unwrap_used,
    reason = "Tests fail immediately on unexpected errors"
)]
use super::{dispatch, presentation::PresentationData, tests::Fixture, Request};
use serde_json::json;

#[test]
fn presentation_roundtrip_preserves_chats_and_root_deck() {
    // Given an old metadata file without presentation.
    let f = Fixture::new();
    std::fs::create_dir(f.root.join(".ai-excalidraw")).unwrap();
    std::fs::write(
        f.root.join(".ai-excalidraw/project.json"),
        r#"{"chats":{"A.excalidraw":[{"id":"legacy"}]}}"#,
    )
    .unwrap();
    assert_eq!(
        f.call("read_presentation", json!({})).unwrap(),
        json!({"decks": {}, "areas": {}})
    );
    let presentation = json!({"decks": {"": {"order": ["A.excalidraw"], "includeSubfolders": false}}, "areas": {"A.excalidraw": {"x": 1, "y": 2, "width": 3, "height": 4}}});
    // When the wire request saves and a fresh registry-backed request reloads.
    assert_eq!(
        f.write_presentation(&presentation).unwrap(),
        json!({"saved": true})
    );
    let read = f.call("read_presentation", json!({})).unwrap();
    // Then the direct response matches persisted data and legacy chats survive.
    let stored = f.snapshot("roundtrip");
    assert_eq!(read, stored["presentation"]);
    assert_eq!(read["decks"], presentation["decks"]);
    assert_eq!(read["areas"]["A.excalidraw"]["width"].as_f64(), Some(3.0));
    assert_eq!(stored["chats"]["A.excalidraw"][0]["id"], "legacy");
}

#[test]
fn writing_chats_keeps_presentation() {
    let (f, _) = Fixture::with_presentation();
    let before = f.call("read_presentation", json!({})).unwrap();
    f.call(
        "write_chats",
        json!({"path": "模块/B.excalidraw", "chats": [{"id": "new"}]}),
    )
    .unwrap();
    let stored = f.snapshot("chat-update");
    assert_eq!(stored["presentation"], before);
    assert_eq!(stored["chats"]["模块/B.excalidraw"][0]["id"], "new");
}

#[test]
fn nested_folder_rename_migrates_all_metadata() {
    let (f, _) = Fixture::with_presentation();
    f.call("rename_file", json!({"from": "模块", "to": "业务"}))
        .unwrap();
    let stored = f.snapshot("folder-rename");
    let p = &stored["presentation"];
    assert_eq!(
        p["decks"][""]["order"],
        json!(["模块外.excalidraw", "业务/子目录/A.excalidraw"])
    );
    assert_eq!(
        p["decks"]["业务"]["order"],
        json!(["业务/B.excalidraw", "业务/子目录/A.excalidraw"])
    );
    assert_eq!(
        p["decks"]["业务/子目录"]["order"],
        json!(["业务/子目录/A.excalidraw"])
    );
    assert!(p["decks"].get("模块").is_none());
    assert!(p["decks"].get("模块/子目录").is_none());
    assert_eq!(p["areas"].as_object().unwrap().len(), 2);
    assert!(p["areas"].get("业务/子目录/A.excalidraw").is_some());
    assert!(p["areas"].get("模块外.excalidraw").is_some());
    assert_eq!(
        stored["chats"]["业务/子目录/A.excalidraw"][0]["id"],
        "original"
    );
    assert!(f.root.join("业务/子目录/A.excalidraw").is_file());
}

#[test]
fn canvas_rename_updates_orders_and_area() {
    let (f, _) = Fixture::with_presentation();
    f.call(
        "rename_file",
        json!({"from": "模块/子目录/A.excalidraw", "to": "模块/子目录/C.excalidraw"}),
    )
    .unwrap();
    let stored = f.snapshot("canvas-rename");
    for deck in stored["presentation"]["decks"]
        .as_object()
        .unwrap()
        .values()
    {
        assert!(deck["order"]
            .as_array()
            .unwrap()
            .contains(&json!("模块/子目录/C.excalidraw")));
        assert!(!deck["order"]
            .as_array()
            .unwrap()
            .contains(&json!("模块/子目录/A.excalidraw")));
    }
    assert!(stored["presentation"]["areas"]
        .get("模块/子目录/C.excalidraw")
        .is_some());
    assert!(stored["presentation"]["areas"]
        .get("模块/子目录/A.excalidraw")
        .is_none());
    assert!(f.root.join("模块/子目录/C.excalidraw").is_file());
}

#[test]
fn duplicate_copies_area_without_order_or_chats() {
    let (f, _) = Fixture::with_presentation();
    let before = f.snapshot("duplicate-before");
    f.call(
        "duplicate_canvas",
        json!({"from": "模块/子目录/A.excalidraw", "to": "副本.excalidraw"}),
    )
    .unwrap();
    let after = f.snapshot("duplicate-after");
    assert_eq!(
        after["presentation"]["decks"],
        before["presentation"]["decks"]
    );
    assert_eq!(after["chats"], before["chats"]);
    assert_eq!(
        after["presentation"]["areas"]["副本.excalidraw"],
        before["presentation"]["areas"]["模块/子目录/A.excalidraw"]
    );
    assert_eq!(
        std::fs::read(f.root.join("副本.excalidraw")).unwrap(),
        std::fs::read(f.root.join("模块/子目录/A.excalidraw")).unwrap()
    );
}

#[test]
fn delete_file_and_folder_removes_only_associated_metadata() {
    for path in ["模块/子目录/A.excalidraw", "模块"] {
        let (f, _) = Fixture::with_presentation();
        let result = f.call("delete_file", json!({"path": path})).unwrap();
        let stored = f.snapshot(if path == "模块" {
            "delete-folder"
        } else {
            "delete-canvas"
        });
        assert_eq!(result, json!({"trashed": true}));
        assert!(!f.root.join(path).exists());
        assert_eq!(stored["chats"], json!({}));
        assert_eq!(
            stored["presentation"]["decks"][""]["order"],
            json!(["模块外.excalidraw"])
        );
        assert_eq!(
            stored["presentation"]["areas"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["模块外.excalidraw"]
        );
        assert_eq!(
            stored["presentation"]["decks"].as_object().unwrap().len(),
            if path == "模块" { 1 } else { 3 }
        );
    }
}

#[test]
fn invalid_paths_bounds_and_limits_leave_disk_unchanged() {
    let (f, _) = Fixture::with_presentation();
    let before = std::fs::read(f.root.join(".ai-excalidraw/project.json")).unwrap();
    for path in [
        "../escape.excalidraw",
        "/tmp/escape.excalidraw",
        "a/../b.excalidraw",
        "a/./b.excalidraw",
        "a//b.excalidraw",
        ".hidden/a.excalidraw",
        "C:\\a.excalidraw",
        "a\0.excalidraw",
        "a/",
    ] {
        for presentation in [
            json!({"decks": {path: {"order": [], "includeSubfolders": false}}}),
            json!({"decks": {"": {"order": [path], "includeSubfolders": false}}}),
            json!({"areas": {path: {"x": 0, "y": 0, "width": 1, "height": 1}}}),
        ] {
            assert!(
                f.write_presentation(&presentation).is_err(),
                "{presentation}"
            );
        }
    }
    for field in ["x", "y", "width", "height"] {
        for value in [json!(null), json!("NaN"), json!(1e10)] {
            let mut area = json!({"x": 0, "y": 0, "width": 1, "height": 1});
            area[field] = value;
            assert!(f
                .call(
                    "write_presentation",
                    json!({"presentation": {"areas": {"A.excalidraw": area}}})
                )
                .is_err());
        }
    }
    for field in ["width", "height"] {
        for value in [0, -1] {
            let mut area = json!({"x": 0, "y": 0, "width": 1, "height": 1});
            area[field] = json!(value);
            assert!(f
                .call(
                    "write_presentation",
                    json!({"presentation": {"areas": {"A.excalidraw": area}}})
                )
                .is_err());
        }
    }
    for presentation in [
        json!({"decks": {"": {"order": [""], "includeSubfolders": false}}}),
        json!({"areas": {"": {"x": 0, "y": 0, "width": 1, "height": 1}}}),
        json!({"decks": (0..5001).map(|n| (format!("folder{n}"), json!({"order": [], "includeSubfolders": false}))).collect::<serde_json::Map<_, _>>()}),
        json!({"areas": (0..5001).map(|n| (format!("{n}.excalidraw"), json!({"x": 0, "y": 0, "width": 1, "height": 1}))).collect::<serde_json::Map<_, _>>()}),
        json!({"decks": {"": {"order": ["file.txt"], "includeSubfolders": true}}}),
        json!({"areas": {"file.txt": {"x": 0, "y": 0, "width": 1, "height": 1}}}),
        json!({"decks": {"": {"order": vec!["A.excalidraw"; 25_001], "includeSubfolders": true}}}),
        json!({"decks": {"x".repeat(4097): {"order": [], "includeSubfolders": true}}}),
    ] {
        assert!(f.write_presentation(&presentation).is_err());
    }
    assert_eq!(
        std::fs::read(f.root.join(".ai-excalidraw/project.json")).unwrap(),
        before
    );
    f.snapshot("rejected-inputs-unchanged");
}

#[test]
fn nonfinite_native_values_are_rejected() {
    let (f, value) = Fixture::with_presentation();
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut presentation: PresentationData = serde_json::from_value(value.clone()).unwrap();
        presentation.areas.get_mut("模块外.excalidraw").unwrap().x = number;
        assert!(dispatch(
            &f.data,
            Request::WritePresentation {
                project_id: f.id.clone(),
                presentation
            }
        )
        .is_err());
    }
    f.snapshot("nonfinite-rejected");
}

#[cfg(unix)]
#[test]
fn symlink_metadata_paths_are_rejected() {
    let (f, _) = Fixture::with_presentation();
    std::os::unix::fs::symlink(&f.data, f.root.join("escape")).unwrap();
    assert!(f
        .call(
            "write_presentation",
            json!({"presentation": {"decks": {"escape": {"order": [], "includeSubfolders": true}}}})
        )
        .is_err());
    assert!(f.call("write_presentation", json!({"presentation": {"areas": {"escape/A.excalidraw": {"x": 0, "y": 0, "width": 1, "height": 1}}}})).is_err());
    f.snapshot("symlink-rejected");
}
