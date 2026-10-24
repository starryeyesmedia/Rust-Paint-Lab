use paint_document::{Engine, recent::Recent, recovery, storage::Store};
fn directory(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("rpl-pref-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn recent_documents_are_bounded_deduplicated_and_reopenable() {
    let dir = directory("recent");
    let mut recent = Recent::default();
    let e = Engine::new(32, 32, 300).unwrap();
    for i in 0..25 {
        let path = dir.join(format!("{i}.rpl"));
        Store::new(path.clone()).save(e.state()).unwrap();
        recent.remember(&path).unwrap();
    }
    assert_eq!(recent.paths.len(), 20);
    recent.remember(&dir.join("24.rpl")).unwrap();
    assert_eq!(recent.paths.len(), 20);
    let p = dir.join("recent.json");
    recent.save(&p).unwrap();
    let loaded = Recent::load(&p).unwrap();
    assert_eq!(loaded.paths, recent.paths);
    std::fs::write(&p, b"not json").unwrap();
    assert!(Recent::load(&p).is_err());
    std::fs::write(&p, vec![b' '; 131073]).unwrap();
    assert!(Recent::load(&p).is_err());
}
#[test]
fn recovery_cleanup_refuses_active_owner_and_arbitrary_artwork() {
    let dir = directory("cleanup");
    let e = Engine::new(32, 32, 300).unwrap();
    let r = recovery::Recovery::start(&dir).unwrap();
    let path = r.path.clone();
    r.flush(e.state().clone()).unwrap();
    assert!(recovery::remove(&dir, &path).is_err());
    assert!(path.exists());
    drop(r);
    assert!(recovery::available(&dir).contains(&path));
    recovery::remove(&dir, &path).unwrap();
    assert!(!path.exists());
    let artwork = dir.join("important.rpl");
    Store::new(artwork.clone()).save(e.state()).unwrap();
    assert!(recovery::remove(&dir, &artwork).is_err());
    assert!(artwork.exists());
}
#[test]
fn recovery_listing_is_bounded_and_does_not_hide_older_inactive_files() {
    let dir = directory("list");
    let e = Engine::new(32, 32, 300).unwrap();
    let mut owners = Vec::new();
    for _ in 0..25 {
        let r = recovery::Recovery::start(&dir).unwrap();
        r.flush(e.state().clone()).unwrap();
        owners.push(r);
    }
    assert!(recovery::available(&dir).is_empty());
    drop(owners.remove(0));
    assert_eq!(recovery::available(&dir).len(), 1);
    drop(owners);
    assert_eq!(recovery::available(&dir).len(), 20);
}

#[test]
fn corrupted_recovery_falls_back_to_verified_backup_without_modifying_either() {
    let dir = directory("fallback");
    let mut e = Engine::new(32, 32, 300).unwrap();
    let r = recovery::Recovery::start(&dir).unwrap();
    r.flush(e.state().clone()).unwrap();
    e.add_layer().unwrap();
    r.flush(e.state().clone()).unwrap();
    let p = r.path.clone();
    drop(r);
    assert_eq!(recovery::restore(&p).unwrap().0.layers.len(), 2);
    // Exercise the verified snapshot-backup fallback independently from the
    // append-only recovery journal, which can otherwise restore the latest state.
    std::fs::remove_file(p.with_extension("rpl.journal")).unwrap();
    std::fs::write(&p, b"interrupted or corrupted snapshot").unwrap();
    let (state, backup) = recovery::restore(&p).unwrap();
    assert!(backup);
    assert_eq!(state.layers.len(), 1);
    assert_eq!(
        std::fs::read(&p).unwrap(),
        b"interrupted or corrupted snapshot"
    );
    std::fs::write(p.with_extension("rpl.bak"), b"also corrupt").unwrap();
    assert!(recovery::restore(&p).is_err());
}

#[test]
fn orphaned_backup_is_discovered_recoverable_and_can_be_cleaned_up() {
    let dir = directory("orphaned-backup");
    let e = Engine::new(32, 32, 300).unwrap();
    let r = recovery::Recovery::start(&dir).unwrap();
    r.flush(e.state().clone()).unwrap();
    r.flush(e.state().clone()).unwrap();
    let path = r.path.clone();
    drop(r);
    std::fs::remove_file(path.with_extension("rpl.journal")).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(recovery::available(&dir).contains(&path));
    let (_, backup) = recovery::restore(&path).unwrap();
    assert!(backup);
    recovery::remove(&dir, &path).unwrap();
    assert!(!path.with_extension("rpl.bak").exists());
}
