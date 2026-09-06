//! Shared test-only campaign persistence. Never compiled into the CLI/library release.
use proptest::{
    prelude::*,
    test_runner::{Config, RngSeed, TestRunner},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Case {
    pub kind: String,
    pub files: BTreeMap<String, Vec<u8>>,
}
pub(crate) fn config(default: u32) -> Config {
    Config {
        cases: std::env::var("M6_TEST_CASES")
            .ok()
            .map(|s| s.parse().expect("M6_TEST_CASES integer"))
            .unwrap_or(default),
        rng_seed: RngSeed::Fixed(
            std::env::var("M6_TEST_SEED")
                .ok()
                .map(|s| s.parse().expect("M6_TEST_SEED integer"))
                .unwrap_or(0x4D3643),
        ),
        failure_persistence: None,
        ..Config::default()
    }
}
pub(crate) fn directory(label: &str) -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let parent = std::env::var_os("M6_TEST_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/m6-campaign"));
    fs::create_dir_all(&parent).unwrap();
    let path = parent.join(format!(
        "{label}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path.canonicalize().unwrap()
}
pub(crate) fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let m = fs::symlink_metadata(&p).unwrap();
            assert!(!m.file_type().is_symlink());
            if m.is_dir() {
                visit(root, &p, map)
            } else {
                map.insert(
                    p.strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                    fs::read(&p).unwrap(),
                );
            }
        }
    }
    let mut map = BTreeMap::new();
    visit(root, root, &mut map);
    map
}
pub(crate) fn restore(case: &Case, root: &Path) {
    assert!(case.files.len() <= 32);
    for (name, bytes) in &case.files {
        let relative = Path::new(name);
        assert!(
            relative
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        );
        assert!(!name.contains(':') && bytes.len() <= 3 * 1024 * 1024);
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        use std::io::Write;
        fs::File::create_new(path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}
pub(crate) fn cleanup(root: &Path) {
    assert!(root.is_absolute() && root.file_name().unwrap().to_string_lossy().contains('-'));
    assert_eq!(root.canonicalize().unwrap(), root);
    fn no_links(p: &Path) {
        let m = fs::symlink_metadata(p).unwrap();
        assert!(!m.file_type().is_symlink());
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            assert_eq!(m.file_attributes() & 0x400, 0);
        }
        if m.is_dir() {
            for e in fs::read_dir(p).unwrap() {
                no_links(&e.unwrap().path());
            }
        }
    }
    no_links(root);
    fs::remove_dir_all(root).unwrap();
}
pub(crate) fn run(label: &str, build: impl Fn(&[u8]) -> Case, check: impl Fn(&Case, &Path)) {
    let evidence = directory(label);
    let mut runner = TestRunner::new(config(8));
    let exercise = |case: Case| {
        assert_eq!(case.kind, label);
        let root = directory("case");
        restore(&case, &root);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&case, &root)));
        if result.is_err() {
            let file = fs::File::create_new(evidence.join(format!(
                "failure-{}.json",
                root.file_name().unwrap().to_string_lossy()
            )))
            .unwrap();
            serde_json::to_writer(file, &case).unwrap();
            return Err(TestCaseError::fail(format!(
                "concrete failure saved in {}",
                evidence.display()
            )));
        }
        cleanup(&root);
        Ok(())
    };
    if let Some(path) = std::env::var_os("M6_TEST_REPLAY").filter(|path| !path.is_empty()) {
        let file = fs::File::open(path).unwrap();
        assert!(file.metadata().unwrap().len() <= 16 * 1024 * 1024);
        let case: Case = serde_json::from_reader(file).unwrap();
        exercise(case).unwrap();
        println!("M6_REPLAY_PASS {label}");
        return;
    }
    // Deterministic seed corpus is replayed on every run before generated cases.
    let corpus: Vec<Vec<u8>> =
        serde_json::from_str(include_str!("../tests/fixtures/m6/mutations.json")).unwrap();
    // Keep one small concrete case on success too, so the replay gate is executable.
    let representative = build(corpus.first().expect("seed corpus is nonempty"));
    serde_json::to_writer(
        fs::File::create_new(evidence.join("replay.json")).unwrap(),
        &representative,
    )
    .unwrap();
    for seed in corpus {
        exercise(build(&seed)).unwrap();
    }
    let count = std::cell::Cell::new(0_u64);
    let result = runner.run(&prop::collection::vec(any::<u8>(), 1..33), |bytes| {
        count.set(count.get() + 1);
        println!("M6_CASE {label} {}", count.get());
        exercise(build(&bytes))
    });
    println!(
        "M6_CAMPAIGN {label} cases={} seed={:?} result={}",
        count.get(),
        runner.config().rng_seed,
        result.is_ok()
    );
    result.unwrap();
    println!("M6_REPLAY_CASE {}", evidence.join("replay.json").display());
}
