use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

pub use anyhow::Result;

pub struct BenchGroupEntry {
    pub name: &'static str,
    pub bench_names: Vec<&'static str>,
    pub runner: fn(&str) -> Result<()>,
}

static BENCH_REGISTRY: LazyLock<Mutex<RefCell<HashMap<&'static str, BenchGroupEntry>>>> =
    LazyLock::new(|| Mutex::new(RefCell::new(HashMap::default())));

pub fn run_bench(group: &str, bench: &str) -> Result<()> {
    let registry = BENCH_REGISTRY.lock().unwrap();
    let registry = registry.borrow();
    if let Some(entry) = registry.get(group) {
        (entry.runner)(bench)
    } else {
        anyhow::bail!("Unknown group: {}", group);
    }
}

pub fn register_group(name: &'static str, entry: BenchGroupEntry) {
    // Check for conflicts
    let map = BENCH_REGISTRY.lock().unwrap();
    let mut map = map.borrow_mut();

    map.entry(name)
        .and_modify(|_| panic!("Duplicate benchmark group registration: {}", name))
        .or_insert(entry);
}

pub fn is_valid_bench(group: &'static str, bench: &'static str) -> bool {
    BENCH_REGISTRY
        .lock()
        .unwrap()
        .borrow()
        .get(group)
        .map(|g| g.bench_names.contains(&bench))
        .unwrap_or(false)
}

pub fn is_valid_group(group: &'static str) -> bool {
    BENCH_REGISTRY.lock().unwrap().borrow().contains_key(group)
}

pub fn list_groups() -> Vec<&'static str> {
    BENCH_REGISTRY
        .lock()
        .unwrap()
        .borrow()
        .keys()
        .cloned()
        .collect()
}

pub fn list_benches(group: &str) -> Result<Vec<&'static str>> {
    // Take what we need under the lock, but don't format errors yet
    if let Some(benches) = {
        BENCH_REGISTRY
            .lock()
            .unwrap()
            .borrow()
            .get(group)
            .map(|g| g.bench_names.clone())
    } {
        return Ok(benches);
    }
    // Lock is dropped here before we call list_groups
    let groups = list_groups();
    anyhow::bail!(
        "Unknown group: {}, the available groups are: {:?}",
        group,
        groups
    );
}
