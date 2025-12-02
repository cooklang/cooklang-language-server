//! Line endings detection and normalization.
//!
//! Adapted from rust-analyzer. We normalize all line endings to `\n` internally
//! and track the original endings to convert back when needed.

/// Represents the line endings style of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEndings {
    /// Unix-style line endings (`\n`)
    #[default]
    Unix,
    /// DOS/Windows-style line endings (`\r\n`)
    Dos,
}

impl LineEndings {
    /// Normalize line endings in `src`, converting `\r\n` to `\n`.
    /// Returns the normalized string and the detected line endings style.
    pub fn normalize(src: String) -> (String, LineEndings) {
        if !src.contains("\r\n") {
            return (src, LineEndings::Unix);
        }

        let normalized = src.replace("\r\n", "\n");
        (normalized, LineEndings::Dos)
    }

    /// Convert a normalized string back to the original line endings style.
    pub fn apply(&self, src: &str) -> String {
        match self {
            LineEndings::Unix => src.to_string(),
            LineEndings::Dos => src.replace('\n', "\r\n"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_unchanged() {
        let src = "line1\nline2\nline3\n";
        let (result, endings) = LineEndings::normalize(src.to_string());
        assert_eq!(endings, LineEndings::Unix);
        assert_eq!(result, src);
    }

    #[test]
    fn test_dos_normalized() {
        let src = "line1\r\nline2\r\nline3\r\n";
        let (result, endings) = LineEndings::normalize(src.to_string());
        assert_eq!(endings, LineEndings::Dos);
        assert_eq!(result, "line1\nline2\nline3\n");
    }

    #[test]
    fn test_mixed_normalized() {
        let src = "line1\r\nline2\nline3\r\n";
        let (result, endings) = LineEndings::normalize(src.to_string());
        assert_eq!(endings, LineEndings::Dos);
        assert_eq!(result, "line1\nline2\nline3\n");
    }

    #[test]
    fn test_apply_unix() {
        let src = "line1\nline2\n";
        assert_eq!(LineEndings::Unix.apply(src), src);
    }

    #[test]
    fn test_apply_dos() {
        let src = "line1\nline2\n";
        assert_eq!(LineEndings::Dos.apply(src), "line1\r\nline2\r\n");
    }
}
