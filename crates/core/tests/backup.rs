use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use stormbuffer_core::{
    ExistingRecordPolicy, GcOptions, IdCollisionPolicy, ImportOptions, MAX_EXPORT_ARCHIVE_BYTES,
    RecordRepository, ScopeCollisionPolicy, StoreInitMode, StorePaths, StoreScope, decode_export,
    encode_export, export_store, gc_store, import_store, initialize_store, parse_markdown,
    render_markdown, write_export_archive,
};

fn temporary_root(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("stormbuffer-backup-{name}-{suffix}"));
    fs::create_dir_all(&root).expect("create temporary root");
    root
}

fn paths(root: &Path, scope: StoreScope) -> StorePaths {
    StorePaths {
        scope,
        root: root.join(".sbuf"),
        records: root.join(".sbuf/records"),
        cache: root.join("cache"),
    }
}

fn fixture() -> stormbuffer_core::Record {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid/fact.md");
    let markdown = fs::read_to_string(&path).expect("read fixture");
    parse_markdown(&path, &markdown).expect("parse fixture")
}

#[test]
fn export_import_preserves_canonical_markdown_and_provenance() {
    let root = temporary_root("round-trip");
    let source_paths = paths(&root.join("source"), StoreScope::Global);
    let target_paths = paths(&root.join("target"), StoreScope::Global);
    initialize_store(&source_paths, StoreInitMode::Default).expect("initialize source");
    initialize_store(&target_paths, StoreInitMode::Default).expect("initialize target");

    let source = RecordRepository::new(source_paths.clone())
        .add(fixture())
        .expect("add fixture");
    let bundle = export_store(&source_paths).expect("export store");
    let encoded = encode_export(&bundle).expect("encode export");
    let decoded = decode_export(&encoded).expect("decode export");
    let report =
        import_store(&target_paths, &decoded, &ImportOptions::default()).expect("import store");
    assert_eq!(report.imported, 1);

    let imported = RecordRepository::new(target_paths.clone())
        .find(source.record().id)
        .expect("find imported record");
    assert_eq!(imported.markdown(), source.markdown());
    assert_eq!(imported.record().sources, source.record().sources);

    let collision = import_store(&target_paths, &decoded, &ImportOptions::default())
        .expect_err("reimport must require a collision policy")
        .to_string();
    assert!(collision.contains("existing-record"), "{collision}");
    let skipped = import_store(
        &target_paths,
        &decoded,
        &ImportOptions {
            existing_record: Some(ExistingRecordPolicy::Skip),
            ..ImportOptions::default()
        },
    )
    .expect("skip existing record");
    assert_eq!(skipped.skipped, 1);

    fs::remove_dir_all(root).expect("remove temporary root");
}

#[test]
fn archives_are_bounded_and_exports_stay_outside_canonical_storage() {
    let root = temporary_root("archive-boundaries");
    let store_paths = paths(&root, StoreScope::Project);
    initialize_store(&store_paths, StoreInitMode::Default).expect("initialize store");

    let oversized = "x".repeat(MAX_EXPORT_ARCHIVE_BYTES + 1);
    let error = decode_export(&oversized)
        .expect_err("oversized archive must fail")
        .to_string();
    assert!(error.contains("byte limit"), "{error}");

    let destination = store_paths.root.join("backup.json");
    let error = write_export_archive(&store_paths, &destination, b"{}")
        .expect_err("export inside the store must fail")
        .to_string();
    assert!(error.contains("outside the selected store"), "{error}");
    assert!(!destination.exists());

    fs::remove_dir_all(root).expect("remove temporary root");
}

#[test]
fn overwrite_equivalent_record_preserves_the_destination_identity() {
    let root = temporary_root("overwrite-identity");
    let source_paths = paths(&root.join("source"), StoreScope::Global);
    let target_paths = paths(&root.join("target"), StoreScope::Global);
    initialize_store(&source_paths, StoreInitMode::Default).expect("initialize source");
    initialize_store(&target_paths, StoreInitMode::Default).expect("initialize target");

    let source = RecordRepository::new(source_paths.clone())
        .add(fixture())
        .expect("add source fixture");
    let mut equivalent = source.record().clone();
    equivalent.id = stormbuffer_core::RecordId::new_v7();
    let destination = RecordRepository::new(target_paths.clone())
        .add(equivalent)
        .expect("add equivalent destination");

    let report = import_store(
        &target_paths,
        &export_store(&source_paths).expect("export source"),
        &ImportOptions {
            existing_record: Some(ExistingRecordPolicy::Overwrite),
            ..ImportOptions::default()
        },
    )
    .expect("overwrite equivalent record");

    assert_eq!(report.imported, 1);
    assert_eq!(report.overwritten, 1);
    let records = RecordRepository::new(target_paths.clone())
        .list(true)
        .expect("list target records");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].record().id, destination.record().id);
    assert!(
        !target_paths
            .records
            .join(format!("{}.md", source.record().id))
            .exists()
    );

    fs::remove_dir_all(root).expect("remove temporary root");
}

#[test]
fn import_requires_scope_and_id_policies_and_gc_never_touches_records() {
    let root = temporary_root("policies");
    let source_paths = paths(&root.join("source"), StoreScope::Global);
    let project_root = root.join("project");
    let project_paths = paths(&project_root, StoreScope::Project);
    initialize_store(&source_paths, StoreInitMode::Default).expect("initialize source");
    initialize_store(&project_paths, StoreInitMode::Default).expect("initialize project");

    let source = RecordRepository::new(source_paths.clone())
        .add(fixture())
        .expect("add fixture");
    let mut bundle = export_store(&source_paths).expect("export store");
    let mut changed = source.record().clone();
    changed.body.push_str(" Changed in the imported archive.");
    bundle.records[0].markdown = render_markdown(&changed).expect("render changed record");
    let id_error = import_store(&source_paths, &bundle, &ImportOptions::default())
        .expect_err("changed same id must require a policy")
        .to_string();
    assert!(id_error.contains("id collision"), "{id_error}");
    let skipped = import_store(
        &source_paths,
        &bundle,
        &ImportOptions {
            id_collision: Some(IdCollisionPolicy::Skip),
            ..ImportOptions::default()
        },
    )
    .expect("skip id collision");
    assert_eq!(skipped.skipped, 1);

    let scope_error = import_store(
        &project_paths,
        &export_store(&source_paths).expect("export"),
        &ImportOptions::default(),
    )
    .expect_err("scope change must require a policy")
    .to_string();
    assert!(scope_error.contains("scope collision"), "{scope_error}");
    let remapped = import_store(
        &project_paths,
        &export_store(&source_paths).expect("export"),
        &ImportOptions {
            scope_collision: Some(ScopeCollisionPolicy::Remap),
            ..ImportOptions::default()
        },
    )
    .expect("remap project scope");
    assert_eq!(remapped.imported, 1);

    let index = project_paths.root.join("index.sqlite3");
    let model = project_paths.cache.join("models/model.onnx");
    fs::create_dir_all(model.parent().expect("model parent")).expect("create model cache");
    fs::write(&index, b"projection").expect("write index");
    fs::write(&model, b"model").expect("write model");
    let record_path = RecordRepository::new(project_paths.clone())
        .list(true)
        .expect("list records")[0]
        .path()
        .to_path_buf();
    let dry_run = gc_store(&project_paths, GcOptions { dry_run: true }).expect("dry-run gc");
    assert!(dry_run.dry_run);
    assert!(index.is_file());
    assert!(model.is_file());
    assert!(record_path.is_file());
    let actual = gc_store(&project_paths, GcOptions { dry_run: false }).expect("actual gc");
    assert_eq!(actual.removed, 2);
    assert!(!index.exists());
    assert!(!model.exists());
    assert!(record_path.is_file());

    fs::remove_dir_all(root).expect("remove temporary root");
}
