#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::{
    digest,
    fs::{Directory, Tree},
    manifest, service,
    transaction::{self, Contents, Write},
    *,
};
use serde_json::{json, Value};
use std::{fs, path::Path};
fn root() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}
fn canonical(t: &tempfile::TempDir) -> std::path::PathBuf {
    t.path().canonicalize().unwrap()
}
fn skill_text() -> String {
    "---\nname: demo\ndescription: Demo skill\n---\nRead [guide](references/guide.md).".into()
}
fn install(root: &Path, revision: Option<String>, resource: &str) -> Result<Value> {
    service::dispatch(
        root,
        Operation::WriteSkill {
            name: "demo".into(),
            content: skill_text(),
            files: vec![ResourceFile {
                path: "references/guide.md".into(),
                content: resource.into(),
            }],
            expected_revision: revision,
            authorized: true,
        },
    )
}
fn rev(value: &Value) -> String {
    value["revision"].as_str().unwrap().to_owned()
}
#[test]
fn missing_parent_bundle_saved_verified_and_preview_revision_matches() {
    let temp = root();
    let root = canonical(&temp);
    let result = install(&root, None, "guide").unwrap();
    assert_eq!(result["verified"], true);
    assert_eq!(result["files"].as_array().unwrap().len(), 2);
    assert_eq!(
        fs::read_to_string(root.join(".agents/skills/demo/references/guide.md")).unwrap(),
        "guide"
    );
    let preview = service::dispatch(
        &root,
        Operation::Read {
            path: ".agents/skills/demo/SKILL.md".into(),
        },
    )
    .unwrap();
    assert_eq!(preview["revision"], result["revision"]);
    assert_ne!(preview["contentRevision"], preview["revision"]);
}
#[test]
fn optimistic_update_backs_up_entire_bundle_and_stale_resource_edit_conflicts() {
    let temp = root();
    let root = canonical(&temp);
    let first = install(&root, None, "one").unwrap();
    let second = install(&root, Some(rev(&first)), "two").unwrap();
    let backup = root.join(second["backupPath"].as_str().unwrap());
    assert_eq!(
        fs::read_to_string(backup.join("references/guide.md")).unwrap(),
        "one"
    );
    fs::write(
        root.join(".agents/skills/demo/references/guide.md"),
        "external",
    )
    .unwrap();
    assert!(install(&root, Some(rev(&second)), "overwrite")
        .unwrap_err()
        .contains("conflict"));
    assert_eq!(
        fs::read_to_string(root.join(".agents/skills/demo/references/guide.md")).unwrap(),
        "external"
    );
}
#[test]
fn validation_and_staging_failure_never_install_or_partially_update() {
    let temp = root();
    let root = canonical(&temp);
    let first = install(&root, None, "old").unwrap();
    for (content, files) in [
        ("bad".to_owned(), vec![]),
        (
            skill_text(),
            vec![ResourceFile {
                path: "../escape".into(),
                content: "bad".into(),
            }],
        ),
        (
            skill_text(),
            vec![ResourceFile {
                path: "references".into(),
                content: "file collides with directory".into(),
            }],
        ),
        (
            skill_text(),
            vec![ResourceFile {
                path: "references/guide.md".into(),
                content: "a".repeat(262145),
            }],
        ),
        (
            skill_text(),
            vec![ResourceFile {
                path: "SKILL.md".into(),
                content: "bad".into(),
            }],
        ),
    ] {
        assert!(service::dispatch(
            &root,
            Operation::WriteSkill {
                name: "demo".into(),
                content,
                files,
                expected_revision: Some(rev(&first)),
                authorized: true
            }
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(root.join(".agents/skills/demo/references/guide.md")).unwrap(),
            "old"
        );
    }
    let temp2_guard = tempfile::tempdir().unwrap();
    let temp2 = canonical(&temp2_guard);
    assert!(service::dispatch(
        &temp2,
        Operation::WriteSkill {
            name: "demo".into(),
            content: skill_text(),
            files: vec![],
            expected_revision: None,
            authorized: true
        }
    )
    .is_err());
    assert!(!temp2.join(".agents/skills/demo").exists());
}
#[test]
fn injected_failure_before_and_after_publish_rolls_back_full_bundle() {
    let temp = root();
    let root = canonical(&temp);
    let first = install(&root, None, "old").unwrap();
    for point in ["before_publish", "after_publish"] {
        transaction::FAULT.with(|f| *f.borrow_mut() = Some(point));
        let result = install(&root, Some(rev(&first)), "new");
        transaction::FAULT.with(|f| *f.borrow_mut() = None);
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(root.join(".agents/skills/demo/references/guide.md")).unwrap(),
            "old"
        );
        assert_eq!(
            fs::read_dir(root.join(".agents/skills")).unwrap().count(),
            1
        );
    }
}
#[test]
fn new_bundle_post_publish_failure_leaves_no_installed_skill() {
    let temp = root();
    let root = canonical(&temp);
    transaction::FAULT.with(|f| *f.borrow_mut() = Some("after_publish"));
    let result = install(&root, None, "new");
    transaction::FAULT.with(|f| *f.borrow_mut() = None);
    assert!(result.is_err());
    assert!(!root.join(".agents/skills/demo").exists());
}
#[test]
fn symlinks_hardlinks_special_files_and_path_escapes_rejected() {
    let temp = root();
    let root = canonical(&temp);
    let outside_guard = tempfile::tempdir().unwrap();
    let outside = canonical(&outside_guard);
    std::os::unix::fs::symlink(&outside, root.join(".agents")).unwrap();
    assert!(install(&root, None, "bad").is_err());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    fs::remove_file(root.join(".agents")).unwrap();
    install(&root, None, "old").unwrap();
    fs::remove_file(root.join(".agents/skills/demo/references/guide.md")).unwrap();
    fs::write(outside.join("secret"), "secret").unwrap();
    std::os::unix::fs::symlink(
        outside.join("secret"),
        root.join(".agents/skills/demo/references/guide.md"),
    )
    .unwrap();
    assert!(service::dispatch(
        &root,
        Operation::Read {
            path: ".agents/skills/demo/references/guide.md".into()
        }
    )
    .is_err());
    fs::remove_file(root.join(".agents/skills/demo/references/guide.md")).unwrap();
    fs::hard_link(
        outside.join("secret"),
        root.join(".agents/skills/demo/references/guide.md"),
    )
    .unwrap();
    assert!(service::dispatch(
        &root,
        Operation::Read {
            path: ".agents/skills/demo/references/guide.md".into()
        }
    )
    .is_err());
    let _socket = std::os::unix::net::UnixListener::bind(root.join("socket")).unwrap();
    fs::rename(
        root.join("socket"),
        root.join(".agents/skills/demo/references/socket"),
    )
    .unwrap();
    assert!(service::dispatch(
        &root,
        Operation::Read {
            path: ".agents/skills/demo/references/socket".into()
        }
    )
    .is_err());
    for path in [
        ".agents/../secret",
        ".agents/mcp.json",
        "/etc/passwd",
        ".agents/skills//demo/SKILL.md",
    ] {
        assert!(service::dispatch(&root, Operation::Read { path: path.into() }).is_err());
    }
}
#[test]
fn replaced_parent_detected_and_no_outside_writes() {
    let temp = root();
    let root = canonical(&temp);
    fs::create_dir(root.join("skills")).unwrap();
    let directory = Directory::root(&root).unwrap();
    let skills = directory.child("skills", false).unwrap();
    let result = transaction::commit(
        &skills,
        Write {
            name: "demo",
            path: "skills/demo",
            contents: Contents::Skill(Tree::from([("SKILL.md".into(), "text".into())])),
            expected: None,
        },
        || {
            fs::rename(root.join("skills"), root.join("moved")).unwrap();
            std::os::unix::fs::symlink("/tmp", root.join("skills")).unwrap();
            directory.verify_child("skills", &skills)
        },
    );
    assert!(result.is_err());
    assert!(!root.join("moved/demo").exists());
}
#[test]
fn confirmed_memory_only_and_policy_denies_win() {
    let temp = root();
    let root = canonical(&temp);
    let memory = |confirmed| Operation::WriteMemory {
        name: "decision".into(),
        content: "Use project files".into(),
        expected_revision: None,
        authorized: true,
        confirmed,
    };
    assert!(service::dispatch(&root, memory(false)).is_err());
    assert!(!root.join(".agents").exists());
    let saved = service::dispatch(&root, memory(true)).unwrap();
    assert_eq!(saved["verified"], true);
    let agents = Directory::root(&root)
        .unwrap()
        .child(".agents", false)
        .unwrap();
    fs::write(
        root.join(".agents/memory/unconfirmed.json"),
        r#"{"confirmed":false,"content":"must not appear"}"#,
    )
    .unwrap();
    let context = manifest::resolve(Some(&agents), None, &Policy::default()).unwrap();
    assert_eq!(context["memory"].as_array().unwrap().len(), 1);
    assert!(!context.to_string().contains("must not appear"));
    let policy = Policy {
        allow: [Capability::WriteSkill].into(),
        deny: [Capability::WriteSkill].into(),
        ..Policy::default()
    };
    service::dispatch(
        &root,
        Operation::WritePolicy {
            policy,
            expected_revision: None,
            authorized: true,
        },
    )
    .unwrap();
    assert!(install(&root, None, "denied").is_err());
    assert!(!Policy::default().permits(Capability::Mcp, false));
}
#[test]
fn project_first_resolution_preserves_disabled_mcp_and_redacts_credentials() {
    let temp = root();
    let root = canonical(&temp);
    let global_guard = tempfile::tempdir().unwrap();
    let global = canonical(&global_guard);
    install(&root, None, "project").unwrap();
    install(&global, None, "global").unwrap();
    fs::write(global.join(".agents/mcp.json"),r#"{"mcpServers":{"same":{"enabled":true,"command":"secret-command","env":{"TOKEN":"secret-token"}},"global":{"enabled":true,"url":"https://secret"}}}"#).unwrap();
    fs::write(root.join(".agents/mcp.json"),r#"{"mcpServers":{"same":{"enabled":false,"command":"local-secret"},"disabled":{"enabled":true,"disabled":true,"url":"https://secret"},"absent":{"command":"secret"}}}"#).unwrap();
    let project = Directory::root(&root)
        .unwrap()
        .child(".agents", false)
        .unwrap();
    let fallback = Directory::root(&global)
        .unwrap()
        .child(".agents", false)
        .unwrap();
    let value = manifest::resolve(Some(&project), Some(&fallback), &Policy::default()).unwrap();
    assert_eq!(value["skills"].as_array().unwrap().len(), 1);
    assert_eq!(value["skills"][0]["source"], "project");
    for entry in value["mcp"].as_array().unwrap() {
        assert_eq!(entry["enabled"], entry["name"] == "global");
    }
    assert!(!value.to_string().contains("secret"));
    assert!(!value.to_string().contains("TOKEN"));
}
#[test]
fn sha256_matches_known_vectors_and_tree_hash_includes_resources() {
    let first = Tree::from([
        ("SKILL.md".into(), "same".into()),
        ("guide.md".into(), "one".into()),
    ]);
    let second = Tree::from([
        ("SKILL.md".into(), "same".into()),
        ("guide.md".into(), "two".into()),
    ]);
    assert_ne!(
        digest::tree_revision(&first),
        digest::tree_revision(&second)
    );

    assert_eq!(
        digest::revision(b""),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        digest::revision(b"abc"),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        digest::revision(&vec![b'a'; 1000000]),
        "sha256:cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}
#[test]
fn request_schema_and_native_budget_ceiling() {
    let request:Request=serde_json::from_value(json!({"projectId":"p","op":"writeSkill","name":"demo","content":"text","expectedRevision":null,"authorized":true,"files":[{"path":"a.txt","content":"a"}]})).unwrap();
    assert!(matches!(request.operation,Operation::WriteSkill{files,..} if files.len()==1));
    let budget = Budgets {
        max_turns: Some(100),
        repair_limit: Some(1),
        timeout_ms: None,
    }
    .restrict(Budgets {
        max_turns: Some(3),
        repair_limit: Some(2),
        timeout_ms: Some(500),
    });
    assert_eq!(budget.max_turns, Some(3));
    assert_eq!(budget.repair_limit, Some(1));
    assert_eq!(budget.timeout_ms, Some(500));
}
#[test]
fn lock_child_process() {
    if let Some(root) = std::env::var_os("AGENT_PROJECT_LOCK_TEST_ROOT") {
        let directory = Directory::root(Path::new(&root)).unwrap();
        if std::env::var_os("AGENT_PROJECT_LOCK_HELD").is_some() {
            assert!(directory.lock().is_err());
            assert!(install(Path::new(&root), None, "blocked")
                .unwrap_err()
                .contains("busy"));
        } else {
            directory.lock().unwrap();
            std::process::exit(0);
        }
    }
}
#[test]
fn cross_process_lock_excludes_writer_and_releases_on_exit() {
    let temp = root();
    let root = canonical(&temp);
    let directory = Directory::root(&root).unwrap();
    directory.lock().unwrap();
    let child = |held: bool| {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "agent_project::tests::lock_child_process",
                "--nocapture",
            ])
            .env("AGENT_PROJECT_LOCK_TEST_ROOT", &root);
        if held {
            command.env("AGENT_PROJECT_LOCK_HELD", "1");
        }
        command.output().unwrap()
    };
    let blocked = child(true);
    assert!(
        blocked.status.success(),
        "{}",
        String::from_utf8_lossy(&blocked.stderr)
    );
    drop(directory);
    let released = child(false);
    assert!(
        released.status.success(),
        "{}",
        String::from_utf8_lossy(&released.stderr)
    );
    Directory::root(&root).unwrap().lock().unwrap();
}

#[test]
fn transaction_conflict_after_exchange_preserves_concurrent_original() {
    let temp = root();
    let root = canonical(&temp);
    let dir = Directory::root(&root).unwrap();
    dir.write_new("fact.json", "old").unwrap();
    let phase = std::cell::Cell::new(0);
    let result = transaction::commit(
        &dir,
        Write {
            name: "fact.json",
            path: "memory/fact.json",
            contents: Contents::File("new".into()),
            expected: Some(digest::revision(b"old")),
        },
        || {
            phase.set(phase.get() + 1);
            if phase.get() == 2 {
                let backup = dir
                    .entries()?
                    .into_iter()
                    .find(|p| p.starts_with(".backup-"))
                    .unwrap();
                fs::write(root.join(backup), "external edit to displaced inode").unwrap();
            }
            Ok(())
        },
    );
    assert!(result.unwrap_err().contains("rolled back"));
    assert_eq!(
        dir.read("fact.json").unwrap(),
        "external edit to displaced inode"
    );
}
#[test]
fn scalar_and_block_frontmatter_references_and_duplicate_validation() {
    assert!(skill::metadata("---\nname: demo\nname: other\ndescription: text\n---\n").is_err());
    assert_eq!(
        skill::metadata("---\nname: demo\ndescription: >\n  First line\n  second line\n---\n")
            .unwrap()
            .1,
        "First line second line"
    );
    let tree = Tree::from([(
        "SKILL.md".into(),
        "---\nname: demo\ndescription: text\n---\nRead `scripts/tool.py`".into(),
    )]);
    assert!(skill::validate("demo", &tree).is_err());
}
#[test]
fn registered_project_command_dispatch_records_real_artifact() {
    let temp = root();
    let root = canonical(&temp);
    let data = root.join("registry");
    fs::create_dir(&data).unwrap();
    let project = root.join("project");
    fs::create_dir(&project).unwrap();
    let opened = crate::workspace::dispatch(
        &data,
        crate::workspace::Request::OpenProject {
            directory: project.to_string_lossy().into_owned(),
        },
    )
    .unwrap();
    let id = opened
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| {
            opened
                .get("project")
                .and_then(|p| p.get("id"))
                .and_then(Value::as_str)
        })
        .unwrap();
    let request = json!({"projectId":id,"op":"writeSkill","name":"demo","content":skill_text(),"files":[{"path":"references/guide.md","content":"Evidence resource"}],"expectedRevision":null,"authorized":true});
    let artifact =
        super::dispatch(&data, serde_json::from_value(request.clone()).unwrap()).unwrap();
    let preview = super::dispatch(
        &data,
        serde_json::from_value(json!({"projectId":id,"op":"read","path":artifact["path"]}))
            .unwrap(),
    )
    .unwrap();
    let mut manifest = super::dispatch(
        &data,
        serde_json::from_value(json!({"projectId":id,"op":"manifest"})).unwrap(),
    )
    .unwrap();
    assert_eq!(artifact["revision"], preview["revision"]);
    assert_eq!(
        manifest["skills"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == "demo")
            .unwrap()["revision"],
        artifact["revision"]
    );
    if let Some(evidence) = std::env::var_os("AGENT_PROJECT_EVIDENCE") {
        // Capture only this synthetic project's entries, never the user's global Skill metadata.
        for key in ["skills", "mcp"] {
            manifest[key]
                .as_array_mut()
                .unwrap()
                .retain(|entry| entry["source"] == "project");
        }
        let evidence = std::path::PathBuf::from(evidence);
        fs::create_dir_all(&evidence).unwrap();
        fs::write(evidence.join("command-fixture.json"),serde_json::to_vec_pretty(&json!({"request":request,"artifact":artifact,"preview":preview,"manifest":manifest})).unwrap()).unwrap();
        fs::write(
            evidence.join("saved-SKILL.md"),
            fs::read(project.join(".agents/skills/demo/SKILL.md")).unwrap(),
        )
        .unwrap();
        fs::write(
            evidence.join("saved-guide.md"),
            fs::read(project.join(".agents/skills/demo/references/guide.md")).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn omitted_resources_are_preserved_and_memory_updates_require_revision() {
    let temp = root();
    let root = canonical(&temp);
    let first = install(&root, None, "preserved").unwrap();
    let updated = service::dispatch(
        &root,
        Operation::WriteSkill {
            name: "demo".into(),
            content: skill_text() + "\nNew instructions",
            files: vec![],
            expected_revision: Some(rev(&first)),
            authorized: true,
        },
    )
    .unwrap();
    assert_ne!(updated["revision"], first["revision"]);
    assert_eq!(
        fs::read_to_string(root.join(".agents/skills/demo/references/guide.md")).unwrap(),
        "preserved"
    );
    let memory = |expected| Operation::WriteMemory {
        name: "fact".into(),
        content: "confirmed".into(),
        expected_revision: expected,
        authorized: true,
        confirmed: true,
    };
    let first = service::dispatch(&root, memory(None)).unwrap();
    assert!(service::dispatch(&root, memory(None)).is_err());
    let updated = service::dispatch(&root, memory(Some(rev(&first)))).unwrap();
    assert!(root.join(updated["backupPath"].as_str().unwrap()).is_file());
}
#[test]
fn rollback_preserves_invalid_concurrent_published_bytes() {
    let temp = root();
    let root = canonical(&temp);
    let dir = Directory::root(&root).unwrap();
    dir.write_new("fact.json", "old").unwrap();
    let phase = std::cell::Cell::new(0);
    let result = transaction::commit(
        &dir,
        Write {
            name: "fact.json",
            path: "memory/fact.json",
            contents: Contents::File("new".into()),
            expected: Some(digest::revision(b"old")),
        },
        || {
            phase.set(phase.get() + 1);
            if phase.get() == 2 {
                fs::write(root.join("fact.json"), [255_u8]).unwrap();
            }
            Ok(())
        },
    );
    assert!(result.unwrap_err().contains("rolled back"));
    assert_eq!(dir.read("fact.json").unwrap(), "old");
    let backup = dir
        .entries()
        .unwrap()
        .into_iter()
        .find(|p| p.starts_with(".backup-"))
        .unwrap();
    assert_eq!(fs::read(root.join(backup)).unwrap(), [255]);
}

#[test]
fn crash_child_process() {
    if let Some(root) = std::env::var_os("AGENT_PROJECT_CRASH_ROOT") {
        let root = Path::new(&root);
        let preview = service::dispatch(
            root,
            Operation::Read {
                path: ".agents/skills/demo/SKILL.md".into(),
            },
        )
        .unwrap();
        let point = if std::env::var_os("AGENT_PROJECT_CRASH_BEFORE").is_some() {
            "before_publish"
        } else {
            "after_publish"
        };
        transaction::CRASH_AT.with(|f| *f.borrow_mut() = Some(point));
        install(root, Some(rev(&preview)), "new after crash").unwrap();
        panic!("crash injection was not reached");
    }
}
#[test]
fn process_crash_has_atomic_publication_and_releases_lock() {
    for before in [true, false] {
        let temp = root();
        let root = canonical(&temp);
        install(&root, None, "old before crash").unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "agent_project::tests::crash_child_process",
                "--nocapture",
            ])
            .env("AGENT_PROJECT_CRASH_ROOT", &root);
        if before {
            child.env("AGENT_PROJECT_CRASH_BEFORE", "1");
        }
        let result = child.output().unwrap();
        assert_eq!(result.status.code(), Some(91));
        let preview = service::dispatch(
            &root,
            Operation::Read {
                path: ".agents/skills/demo/SKILL.md".into(),
            },
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(root.join(".agents/skills/demo/references/guide.md")).unwrap(),
            if before {
                "old before crash"
            } else {
                "new after crash"
            }
        );
        assert_eq!(preview["content"], skill_text());
        install(&root, Some(rev(&preview)), "subsequent successful write").unwrap();
    }
}

#[test]
fn malformed_mcp_enablement_fails_closed() {
    let temp = root();
    let root = canonical(&temp);
    fs::create_dir(root.join(".agents")).unwrap();
    fs::write(
        root.join(".agents/mcp.json"),
        r#"{"mcpServers":{"bad":{"enabled":true,"disabled":"true","command":"private"}}}"#,
    )
    .unwrap();
    let agents = Directory::root(&root)
        .unwrap()
        .child(".agents", false)
        .unwrap();
    assert!(manifest::resolve(Some(&agents), None, &Policy::default()).is_err());
}

#[test]
fn global_skill_read_is_manifest_scoped_and_project_first() {
    let project_temp = root();
    let project_root = canonical(&project_temp);
    let global_temp = root();
    let global_root = canonical(&global_temp);
    install(&global_root, None, "global guide").unwrap();
    let global = Directory::root(&global_root)
        .unwrap()
        .child(".agents", false)
        .unwrap();
    let context = manifest::resolve(None, Some(&global), &Policy::default()).unwrap();
    let skill_path = context["skills"][0]["path"].as_str().unwrap();
    assert_eq!(skill_path, "global-skill://demo/SKILL.md");
    let instructions =
        manifest::read_global_from(None, &global, &Policy::default(), skill_path).unwrap();
    assert_eq!(instructions["content"], skill_text());
    assert_eq!(instructions["revision"], context["skills"][0]["revision"]);
    let guide = manifest::read_global_from(
        None,
        &global,
        &Policy::default(),
        "global-skill://demo/references/guide.md",
    )
    .unwrap();
    assert_eq!(guide["content"], "global guide");
    for path in [
        "global-skill://demo/../mcp.json",
        "global-skill://demo/.hidden",
        "global-skill://other/SKILL.md",
        "global-skill://demo/missing.md",
        "global-skill://demo//SKILL.md",
        "global-skill://demo/references/../../policy.json",
        "global-skill:///etc/passwd",
        "/etc/passwd",
    ] {
        assert!(
            manifest::read_global_from(None, &global, &Policy::default(), path).is_err(),
            "{path}"
        );
    }
    let policy = Policy {
        deny: [Capability::Read].into(),
        ..Policy::default()
    };
    assert!(manifest::read_global_from(None, &global, &policy, skill_path).is_err());
    install(&project_root, None, "project guide").unwrap();
    let project = Directory::root(&project_root)
        .unwrap()
        .child(".agents", false)
        .unwrap();
    assert!(
        manifest::read_global_from(Some(&project), &global, &Policy::default(), skill_path)
            .is_err()
    );
    let context = manifest::resolve(Some(&project), Some(&global), &Policy::default()).unwrap();
    assert_eq!(context["skills"][0]["path"], ".agents/skills/demo/SKILL.md");
}

#[test]
fn global_skill_links_and_global_write_attempts_fail_closed() {
    let global_temp = root();
    let global_root = canonical(&global_temp);
    install(&global_root, None, "original").unwrap();
    let global = Directory::root(&global_root)
        .unwrap()
        .child(".agents", false)
        .unwrap();
    let outside = root();
    fs::write(outside.path().join("secret"), "secret").unwrap();
    fs::remove_file(global_root.join(".agents/skills/demo/references/guide.md")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret"),
        global_root.join(".agents/skills/demo/references/guide.md"),
    )
    .unwrap();
    assert!(manifest::read_global_from(
        None,
        &global,
        &Policy::default(),
        "global-skill://demo/references/guide.md"
    )
    .is_err());
    let project_temp = root();
    let project_root = canonical(&project_temp);
    assert!(service::dispatch(
        &project_root,
        Operation::WriteSkill {
            name: "global-skill://demo".into(),
            content: skill_text(),
            files: vec![],
            expected_revision: None,
            authorized: true
        }
    )
    .is_err());
    assert_eq!(
        fs::read_to_string(outside.path().join("secret")).unwrap(),
        "secret"
    );
}

#[test]
fn global_dispatch_child_process() {
    let Some(path) = std::env::var_os("AGENT_GLOBAL_READ_PROJECT") else {
        return;
    };
    let root = Path::new(&path);
    let data = root.join("registry");
    fs::create_dir(&data).unwrap();
    let opened = crate::workspace::dispatch(
        &data,
        crate::workspace::Request::OpenProject {
            directory: root.to_string_lossy().into_owned(),
        },
    )
    .unwrap();
    let id = opened["id"].as_str().unwrap();
    let invoke = |operation| {
        super::dispatch(
            &data,
            Request {
                project_id: id.into(),
                operation,
            },
        )
    };
    let manifest = invoke(Operation::Manifest).unwrap();
    let reference = manifest["skills"][0]["path"].as_str().unwrap();
    let instructions = invoke(Operation::Read {
        path: reference.into(),
    })
    .unwrap();
    let resource = invoke(Operation::Read {
        path: "global-skill://demo/references/guide.md".into(),
    })
    .unwrap();
    assert_eq!(instructions["content"], skill_text());
    assert_eq!(resource["content"], "global dispatch guide");
    assert!(!root.join(".agents").exists());
    assert!(invoke(Operation::Read {
        path: std::env::var("HOME").unwrap() + "/.agents/skills/demo/SKILL.md"
    })
    .is_err());
    let denied = Policy {
        deny: [Capability::Read].into(),
        ..Policy::default()
    };
    let policy = invoke(Operation::WritePolicy {
        policy: denied,
        expected_revision: None,
        authorized: true,
    })
    .unwrap();
    assert!(invoke(Operation::Read {
        path: reference.into()
    })
    .is_err());
    invoke(Operation::WritePolicy {
        policy: Policy::default(),
        expected_revision: Some(rev(&policy)),
        authorized: true,
    })
    .unwrap();
    install(root, None, "project override").unwrap();
    assert!(invoke(Operation::Read {
        path: reference.into()
    })
    .is_err());
    let local = invoke(Operation::Read {
        path: ".agents/skills/demo/references/guide.md".into(),
    })
    .unwrap();
    assert_eq!(local["content"], "project override");
    if let Some(output) = std::env::var_os("AGENT_GLOBAL_READ_EVIDENCE") {
        fs::write(Path::new(&output).join("global-read-fixture.json"),serde_json::to_vec_pretty(&json!({"manifest":manifest,"instructions":instructions,"resource":resource,"projectOverride":local,"readPolicyEnforced":true,"globalReferenceRejectedAfterProjectOverride":true,"absoluteHomePathRejected":true})).unwrap()).unwrap();
    }
}

#[test]
fn global_discovery_and_reads_use_real_registry_command_with_isolated_home() {
    let project_temp = root();
    let project = canonical(&project_temp);
    let home_temp = root();
    let home = canonical(&home_temp);
    install(&home, None, "global dispatch guide").unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "--exact",
            "agent_project::tests::global_dispatch_child_process",
            "--nocapture",
        ])
        .env("HOME", &home)
        .env("AGENT_GLOBAL_READ_PROJECT", &project);
    if let Some(evidence) = std::env::var_os("AGENT_PROJECT_EVIDENCE") {
        child.env("AGENT_GLOBAL_READ_EVIDENCE", evidence);
    }
    let result = child.output().unwrap();
    assert!(
        result.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(home.join(".agents/skills/demo/references/guide.md")).unwrap(),
        "global dispatch guide"
    );
}

#[test]
fn confirmed_memory_credentials_are_rejected_before_any_artifact_is_written() {
    let cases = [
        (
            "pem",
            "-----BEGIN PRIVATE KEY-----\nsynthetic\n-----END PRIVATE KEY-----",
        ),
        ("openssh", "-----BEGIN OPENSSH PRIVATE KEY-----\nsynthetic"),
        (
            "encrypted-pem",
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\nsynthetic",
        ),
        ("bearer-header", "Authorization: Bearer abc123"),
        ("bearer-json", r#"{"Authorization":"Bearer abc123"}"#),
        (
            "pgp-key",
            "-----BEGIN PGP PRIVATE KEY BLOCK-----\nsynthetic",
        ),
        ("bearer-token", "Use Bearer synthetic-token-12345"),
        ("api-env", "OPENAI_API_KEY=synthetic-secret-value"),
        ("api-json", r#"{"apiKey": "synthetic-secret-value"}"#),
        ("api-header", "x-api-key: synthetic-secret-value"),
        ("password", "password = 'synthetic-password'"),
        (
            "cloud-secret",
            "aws_secret_access_key: synthetic-secret-value",
        ),
    ];
    let mut outcomes = Vec::new();
    for (label, content) in cases {
        let temp = root();
        let root = canonical(&temp);
        let result = service::dispatch(
            &root,
            Operation::WriteMemory {
                name: "fact".into(),
                content: content.into(),
                expected_revision: None,
                authorized: true,
                confirmed: true,
            },
        );
        let error = result.unwrap_err();
        assert!(error.contains("apparent credential"), "{label}");
        assert!(!error.contains("synthetic"));
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0, "{label}");
        outcomes.push(json!({"case":label,"rejected":true,"projectFilesWritten":0}));
    }
    if let Some(evidence) = std::env::var_os("AGENT_PROJECT_EVIDENCE") {
        fs::write(
            Path::new(&evidence).join("memory-credential-rejections.json"),
            serde_json::to_vec_pretty(&outcomes).unwrap(),
        )
        .unwrap();
    }
}
#[test]
fn credential_memory_update_preserves_previous_record_without_backup() {
    let temp = root();
    let root = canonical(&temp);
    let write = |content: &str, expected| Operation::WriteMemory {
        name: "fact".into(),
        content: content.into(),
        expected_revision: expected,
        authorized: true,
        confirmed: true,
    };
    let first = service::dispatch(
        &root,
        write(
            "Use the configured API key from the environment; never save its value.",
            None,
        ),
    )
    .unwrap();
    let path = root.join(".agents/memory/fact.json");
    let previous = fs::read(&path).unwrap();
    assert!(service::dispatch(
        &root,
        write("api_key=synthetic-secret-value", Some(rev(&first)))
    )
    .is_err());
    assert_eq!(fs::read(path).unwrap(), previous);
    assert_eq!(
        fs::read_dir(root.join(".agents/memory")).unwrap().count(),
        1
    );
}
#[test]
fn nonsecret_security_facts_and_explicit_placeholders_remain_valid() {
    for content in [
        "Use Bearer authentication for the service.",
        "The API key is stored in the OS keychain.",
        "api_key=${OPENAI_API_KEY}",
        "api_key=<redacted>",
        "api_key=REDACTED",
        r#"{"api_key":null}"#,
        r#"{"api_key":""}"#,
        "has_api_key: true",
        "Authorization: Bearer <token>",
        "A private key must never be stored in project memory.",
    ] {
        assert!(super::memory::validate(content).is_ok(), "{content}");
    }
}

#[test]
fn global_quarantine_is_per_entry_and_descriptors_do_not_read_resource_bodies() {
    let global_temp = root();
    let root = canonical(&global_temp);
    install(&root, None, "guide").unwrap();
    let skills = root.join(".agents/skills");
    fs::create_dir(skills.join("broken")).unwrap();
    fs::write(skills.join("broken/SKILL.md"), "invalid frontmatter").unwrap();
    std::os::unix::fs::symlink("/tmp", skills.join("linked")).unwrap();
    fs::create_dir(skills.join("binary")).unwrap();
    fs::write(
        skills.join("binary/SKILL.md"),
        "---\nname: binary\ndescription: Binary reference\n---\n[asset](asset.bin)",
    )
    .unwrap();
    fs::write(skills.join("binary/asset.bin"), [255, 254, 253]).unwrap();
    let agents = Directory::root(&root)
        .unwrap()
        .child(".agents", false)
        .unwrap();
    let value = manifest::resolve(None, Some(&agents), &Policy::default()).unwrap();
    assert_eq!(value["skills"].as_array().unwrap().len(), 2);
    assert_eq!(value["warnings"].as_array().unwrap().len(), 2);
    let binary = value["skills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "binary")
        .unwrap();
    let resource = binary["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["path"] == "global-skill://binary/asset.bin")
        .unwrap();
    assert!(resource["revision"].is_null());
    assert_eq!(resource["bytes"], 3);
    assert!(manifest::read_global_from(
        None,
        &agents,
        &Policy::default(),
        "global-skill://broken/SKILL.md"
    )
    .is_err());
}
#[test]
fn project_override_is_selected_before_invalid_global_skill_and_mcp_validation() {
    let project_temp = root();
    let project_root = canonical(&project_temp);
    install(&project_root, None, "local").unwrap();
    let global_temp = root();
    let global_root = canonical(&global_temp);
    fs::create_dir_all(global_root.join(".agents/skills")).unwrap();
    std::os::unix::fs::symlink("/tmp", global_root.join(".agents/skills/demo")).unwrap();
    fs::write(
        project_root.join(".agents/mcp.json"),
        r#"{"mcpServers":{"same":{"enabled":false,"command":"local"}}}"#,
    )
    .unwrap();
    fs::write(global_root.join(".agents/mcp.json"),r#"{"mcpServers":{"same":"invalid stale global","other":{"enabled":true,"command":"global"}}}"#).unwrap();
    let project = Directory::root(&project_root)
        .unwrap()
        .child(".agents", false)
        .unwrap();
    let global = Directory::root(&global_root)
        .unwrap()
        .child(".agents", false)
        .unwrap();
    let value = manifest::resolve(Some(&project), Some(&global), &Policy::default()).unwrap();
    assert!(value["warnings"].as_array().unwrap().is_empty());
    assert_eq!(value["skills"][0]["source"], "project");
    assert_eq!(value["mcp"].as_array().unwrap().len(), 2);
    assert_eq!(value["mcp"][1]["enabled"], false);
    fs::write(
        project_root.join(".agents/policy.json"),
        r#"{"deny":"invalid"}"#,
    )
    .unwrap();
    assert!(service::dispatch(&project_root, Operation::Manifest)
        .unwrap_err()
        .contains("invalid project policy"));
}
#[test]
fn large_global_catalog_truncates_with_warning_without_breaking_project_manifest() {
    let temp = root();
    let root = canonical(&temp);
    fs::create_dir_all(root.join("skills")).unwrap();
    for i in 0..80 {
        let name = format!("skill-{i}");
        let path = root.join("skills").join(&name);
        fs::create_dir(&path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!(
                "---\nname: {name}\ndescription: {}\n---\nBody",
                "x".repeat(2000)
            ),
        )
        .unwrap();
    }
    let global = Directory::root(&root).unwrap();
    let value = manifest::resolve(None, Some(&global), &Policy::default()).unwrap();
    assert!(serde_json::to_vec(&value).unwrap().len() <= 131072);
    assert!(value["skills"].as_array().unwrap().len() < 80);
    assert!(!value["warnings"].as_array().unwrap().is_empty());
}
#[test]
fn real_home_manifest_readonly_smoke() {
    if std::env::var("AGENT_REAL_MANIFEST_READONLY").as_deref() != Ok("1") {
        return;
    }
    let home = std::env::var("HOME").unwrap();
    assert!(Path::new(&home).is_absolute() && Path::new(&home).is_dir());
    let project = std::env::var("AGENT_REAL_MANIFEST_PROJECT").unwrap();
    assert!(Path::new(&project).is_absolute() && Path::new(&project).is_dir());
    let result = service::dispatch(Path::new(&project), Operation::Manifest);
    let summary = match &result {
        Ok(value) => {
            let skills = value["skills"].as_array().unwrap();
            let mcp = value["mcp"].as_array().unwrap();
            json!({"status":"ok","operation":"manifest","readOnly":true,"realHomeUsed":true,"skillCount":skills.len(),"projectSkillCount":skills.iter().filter(|v|v["source"]=="project").count(),"globalSkillCount":skills.iter().filter(|v|v["source"]=="global").count(),"mcpCount":mcp.len(),"enabledMcpCount":mcp.iter().filter(|v|v["enabled"]==true).count(),"memoryCount":value["memory"].as_array().unwrap().len(),"serializedBytes":serde_json::to_vec(value).unwrap().len(),"warnings":value["warnings"]})
        }
        Err(error) => {
            let category = [
                "frontmatter required",
                "frontmatter terminator required",
                "invalid frontmatter field",
                "invalid frontmatter key",
                "invalid quoted frontmatter scalar",
                "unsupported frontmatter scalar",
                "duplicate frontmatter field",
                "frontmatter name mismatch",
                "frontmatter",
                "resource",
                "symlink",
                "invalid Skill/memory name",
                "budget",
                "project busy",
                "invalid project policy",
                "denied",
                "No such file",
            ]
            .into_iter()
            .find(|kind| error.contains(kind))
            .unwrap_or("project manifest configuration or path rejected");
            json!({"status":"rejected","operation":"manifest","readOnly":true,"category":category})
        }
    };
    if let Some(evidence) = std::env::var_os("AGENT_PROJECT_EVIDENCE") {
        fs::create_dir_all(&evidence).unwrap();
        fs::write(
            Path::new(&evidence).join("real-home-manifest-smoke.json"),
            serde_json::to_vec_pretty(&summary).unwrap(),
        )
        .unwrap();
    }
    println!("{}", serde_json::to_string(&summary).unwrap());
    assert!(
        result.is_ok(),
        "real manifest smoke rejected; contents deliberately not logged"
    );
}

#[test]
fn optional_nested_skill_metadata_does_not_invalidate_required_fields() {
    let parsed=skill::metadata("---\nname: demo\ndescription: Valid\nmetadata:\n  author: Example\n  tags:\n    - commerce\nallowed-tools: [Read, Write]\n---\nBody").unwrap();
    assert_eq!(parsed, ("demo".into(), "Valid".into()));
    assert!(skill::metadata("---\nname: demo\nmetadata:\n  name: ignored\ndescription: Valid\nname: duplicate\n---\nBody").is_err());
}
