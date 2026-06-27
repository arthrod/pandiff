//! `diffu` (unified line diff for code blocks) and a faithful port of the
//! `wordwrap` npm package (soft mode, used for prose re-wrapping).

use once_cell::sync::Lazy;
use regex::Regex;
use similar::{ChangeTag, TextDiff};

/// Produces a unified-style diff (`-`/`+`/space prefixes) of two texts, line by
/// line. Mirrors the TypeScript `diffu` built on the `diff` package.
pub fn diffu(text1: &str, text2: &str) -> String {
    let diff = TextDiff::from_lines(text1, text2);
    let mut result: Vec<String> = Vec::new();
    for change in diff.iter_all_changes() {
        let prefix = match change.tag() {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };
        let value = change.value();
        // Strip a single trailing newline, then prefix each resulting line.
        let chunk = value.strip_suffix('\n').unwrap_or(value);
        for line in chunk.split('\n') {
            result.push(format!("{prefix}{line}"));
        }
    }
    result.join("\n")
}

// `(\S+\s+)` — a run of non-whitespace followed by trailing whitespace.
static RE_CHUNK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\S+\s+").unwrap());

/// Splits `text` the way JavaScript `String.split(/(\S+\s+)/)` would: the
/// captured separators are interleaved with the inter-match text.
fn split_capturing(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut last = 0;
    for m in RE_CHUNK.find_iter(text) {
        result.push(text[last..m.start()].to_string());
        result.push(text[m.start()..m.end()].to_string());
        last = m.end();
    }
    result.push(text[last..].to_string());
    result
}

/// Soft word-wrap at `columns` (start = 0). Faithful port of `wordwrap(columns)`.
pub fn wordwrap(columns: usize, text: &str) -> String {
    let stop = columns;
    let chunks = split_capturing(text);
    let mut lines: Vec<String> = vec![String::new()];

    for raw in chunks {
        if raw.is_empty() {
            continue;
        }
        let chunk = raw.replace('\t', "    ");
        let i = lines.len() - 1;
        // JS `.length` counts UTF-16 units; char count matches for the BMP text
        // Pandoc emits here.
        if lines[i].chars().count() + chunk.chars().count() > stop {
            let trimmed = lines[i].trim_end().to_string();
            lines[i] = trimmed;
            for c in chunk.split('\n') {
                lines.push(c.trim_start().to_string());
            }
        } else if chunk.contains('\n') {
            let mut xs = chunk.split('\n');
            let first = xs.next().unwrap_or("");
            lines[i].push_str(first);
            for c in xs {
                lines.push(c.trim_start().to_string());
            }
        } else {
            lines[i].push_str(&chunk);
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn diffu_marks_changed_lines() {
        let a = "print(\"Hello\")\nprint(\"world.\")\nprint(\"end\")\n";
        let b = "print(\"Hello\")\nprint(\"world!\")\nprint(\"end\")\n";
        assert_eq!(
            diffu(a, b),
            " print(\"Hello\")\n-print(\"world.\")\n+print(\"world!\")\n print(\"end\")"
        );
    }

    #[test]
    fn diffu_handles_deletion() {
        assert_eq!(diffu("a\nb\nc\n", "a\nc\n"), " a\n-b\n c");
    }

    #[test]
    fn wrap_prose_at_72() {
        let input = "Don’t go around saying to people that the world owes you a living. The world owes you nothing. It was here first.";
        assert_eq!(
            wordwrap(72, input),
            "Don’t go around saying to people that the world owes you a living. The\nworld owes you nothing. It was here first."
        );
    }

    #[test]
    fn wrap_short_line_unchanged() {
        assert_eq!(wordwrap(72, "short line"), "short line");
    }

    #[test]
    fn wrap_long_unbroken_sequence() {
        let input =
            "a b c d e f g h i j k l m n o p q r s t u v w x y z a b c d e f g h i j k l m n o p q r s t u v w x y z";
        assert_eq!(
            wordwrap(72, input),
            "a b c d e f g h i j k l m n o p q r s t u v w x y z a b c d e f g h i j\nk l m n o p q r s t u v w x y z"
        );
    }

    #[test]
    fn wrap_preserves_internal_multiple_spaces_within_limit() {
        assert_eq!(wordwrap(72, "one  two   three"), "one  two   three");
    }
}
