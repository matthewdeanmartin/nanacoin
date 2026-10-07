use super::*;

#[test]
fn publication_replaces_unicode_paths_and_failure_preserves_the_previous_head() {
    let directory = std::env::temp_dir().join(format!("nanacoin-publish-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("next-🎁");
    let head = directory.join("head-中");
    std::fs::write(&head, b"old").unwrap();
    assert!(publish(&source, &head).is_err());
    assert_eq!(std::fs::read(&head).unwrap(), b"old");
    std::fs::write(&source, b"new").unwrap();
    publish(&source, &head).unwrap();
    assert_eq!(std::fs::read(&head).unwrap(), b"new");
    assert!(!source.exists());
    std::fs::write(&source, b"unpublished").unwrap();
    assert!(publish(&source, &directory).is_err());
    assert_eq!(std::fs::read(&source).unwrap(), b"unpublished");
    assert_eq!(std::fs::read(&head).unwrap(), b"new");
    std::fs::remove_file(source).unwrap();
    std::fs::remove_file(head).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[cfg(windows)]
#[test]
fn nul_paths_cannot_publish_to_a_truncated_filename() {
    let directory =
        std::env::temp_dir().join(format!("nanacoin-publish-nul-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("source");
    let target = directory.join("head");
    std::fs::write(&source, b"new").unwrap();
    std::fs::write(&target, b"old").unwrap();
    for (from, to) in [
        (directory.join("source\0suffix"), target.clone()),
        (source.clone(), directory.join("head\0suffix")),
    ] {
        assert_eq!(
            publish(&from, &to).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(std::fs::read(&source).unwrap(), b"new");
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
    }
    std::fs::remove_file(source).unwrap();
    std::fs::remove_file(target).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
