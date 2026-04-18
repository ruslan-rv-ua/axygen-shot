use crate::errors::ShotError;
use std::path::{Path, PathBuf};

pub struct SavedFile {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub filename: String,
}

/// Replace invalid filename characters (and spaces) with hyphens.
/// Characters: : \ / * ? " < > | (space) and control chars (0x00–0x1F)
pub fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ':' | '\\' | '/' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' => '-',
            c if c.is_control() => '-',
            _ => c,
        })
        .collect()
}

/// Build filename: YYYY-MM-DD_HHMMSS-mmm_<sanitized>.png
pub fn build_filename(label: Option<&str>, window_title: &str) -> String {
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S%3f");
    let name_part = match label {
        Some(l) => sanitize_filename(l),
        None => sanitize_filename(window_title),
    };
    format!("{}_{}.png", timestamp, name_part)
}

/// Save PNG bytes to disk. Creates directory if absent.
pub fn save(
    png_bytes: &[u8],
    project_root: &Path,
    folder: &str,
    label: Option<&str>,
    window_title: &str,
) -> Result<SavedFile, ShotError> {
    let dir = project_root.join(folder);
    std::fs::create_dir_all(&dir)
        .map_err(|e| ShotError::StorageFailed(format!("Cannot create {}: {}", dir.display(), e)))?;

    let filename = build_filename(label, window_title);
    let path = dir.join(&filename);

    std::fs::write(&path, png_bytes)
        .map_err(|e| ShotError::StorageFailed(format!("Cannot write {}: {}", path.display(), e)))?;

    Ok(SavedFile { path, filename })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- sanitize_filename ---

    #[test]
    fn sanitize_replaces_special_chars() {
        assert_eq!(sanitize_filename("foo:bar\\baz"), "foo-bar-baz");
    }

    #[test]
    fn sanitize_preserves_normal_chars() {
        assert_eq!(sanitize_filename("hello-world_2024"), "hello-world_2024");
    }

    #[test]
    fn sanitize_all_special_chars() {
        assert_eq!(
            sanitize_filename("a/b*c?d\"e<f>g|h:i\\j k"),
            "a-b-c-d-e-f-g-h-i-j-k",
        );
    }

    #[test]
    fn sanitize_strips_control_chars() {
        assert_eq!(sanitize_filename("app\x00name\x1f"), "app-name-");
    }

    // --- build_filename ---

    #[test]
    fn build_filename_with_label() {
        let name = build_filename(Some("login"), "Ignored Title");
        assert!(name.contains("login"));
        assert!(name.ends_with(".png"));
    }

    #[test]
    fn build_filename_without_label_uses_title() {
        let name = build_filename(None, "My App: v2");
        assert!(name.contains("My-App--v2"));
        assert!(name.ends_with(".png"));
    }

    #[test]
    fn build_filename_has_timestamp_prefix() {
        let name = build_filename(Some("test"), "title");
        // Format: YYYY-MM-DD_HHMMSSmmm_test.png
        assert!(name.chars().nth(4) == Some('-')); // YYYY-
        assert!(name.chars().nth(7) == Some('-')); // MM-
        assert!(name.chars().nth(10) == Some('_')); // DD_
        // Milliseconds: 3 extra digits after SS
        let underscore_pos = name.find("_test.png").unwrap();
        assert_eq!(underscore_pos, 20); // YYYY-MM-DD_HHMMSSmmm = 20 chars
    }

    // --- save ---

    #[test]
    fn save_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let result = save(b"fake png", dir.path(), "output", Some("test"), "Title").unwrap();
        assert!(result.path.exists());
        assert!(dir.path().join("output").is_dir());
    }

    #[test]
    fn save_writes_correct_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let data = b"fake png data";
        let result = save(data, dir.path(), "screenshots", Some("test"), "Title").unwrap();
        assert_eq!(std::fs::read(&result.path).unwrap(), data);
    }

    #[test]
    fn save_filename_contains_label() {
        let dir = tempfile::tempdir().unwrap();
        let result = save(
            b"data",
            dir.path(),
            "screenshots",
            Some("my-label"),
            "Title",
        )
        .unwrap();
        assert!(result.filename.contains("my-label"));
        assert!(result.filename.ends_with(".png"));
    }

    #[test]
    fn two_saves_same_timestamp_dont_panic() {
        let dir = tempfile::tempdir().unwrap();
        let r1 = save(b"one", dir.path(), "screenshots", Some("alpha"), "Title");
        let r2 = save(b"two", dir.path(), "screenshots", Some("beta"), "Title");
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert_ne!(r1.unwrap().filename, r2.unwrap().filename);
    }
}
