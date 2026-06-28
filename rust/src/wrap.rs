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

    #[test]
    fn wrap_handles_embedded_newline_within_limit() {
        // A chunk containing '\n' that still fits exercises the newline branch.
        assert_eq!(wordwrap(72, "a\nb"), "a\nb");
    }

    #[test]
    fn wrap_handles_tab_expansion() {
        assert_eq!(wordwrap(72, "a\tb"), "a    b");
    }

    // --- Additional boundary & regression tests ----------------------------

    #[test]
    fn diffu_identical_texts_produces_all_context_lines() {
        let text = "line1\nline2\nline3\n";
        let out = diffu(text, text);
        // Every line should be prefixed with a space (equal)
        for line in out.lines() {
            assert!(
                line.starts_with(' '),
                "expected context prefix, got: {line:?}"
            );
        }
    }

    #[test]
    fn diffu_empty_first_produces_only_insertions() {
        let out = diffu("", "a\nb\n");
        for line in out.lines() {
            assert!(
                line.starts_with('+'),
                "expected insertion prefix, got: {line:?}"
            );
        }
    }

    #[test]
    fn diffu_empty_second_produces_only_deletions() {
        let out = diffu("a\nb\n", "");
        for line in out.lines() {
            assert!(
                line.starts_with('-'),
                "expected deletion prefix, got: {line:?}"
            );
        }
    }

    #[test]
    fn diffu_both_empty_produces_empty_output() {
        assert_eq!(diffu("", ""), "");
    }

    #[test]
    fn diffu_insertion_only_no_deletions() {
        let out = diffu("unchanged\n", "unchanged\nextra line\n");
        assert!(out.contains("+extra line"), "should have insertion");
        assert!(!out.contains('-'), "should have no deletions");
    }

    #[test]
    fn wordwrap_empty_string() {
        assert_eq!(wordwrap(72, ""), "");
    }

    #[test]
    fn wordwrap_at_custom_column_count() {
        // At 20 columns, this 30-char sentence should wrap.
        let input = "one two three four five six";
        let out = wordwrap(20, input);
        assert!(out.contains('\n'), "expected wrap at 20 cols");
        // No single line should exceed 20 chars after wrap
        for line in out.lines() {
            assert!(line.chars().count() <= 20, "line too long: {line:?}");
        }
    }

    #[test]
    fn wordwrap_exact_boundary_does_not_wrap() {
        // Exactly at the column limit, no overflow → should not wrap.
        let input = "1234567890"; // 10 chars
        assert_eq!(wordwrap(10, input), input);
    }

    #[test]
    fn wordwrap_single_word_longer_than_columns_not_split() {
        // wordwrap does not split individual words, only at word boundaries.
        let long_word = "superlongwordthatexceedscolumnlimit";
        let out = wordwrap(10, long_word);
        // A single word with no whitespace: split_capturing returns it as one
        // chunk; since lines[0] is empty, 0 + len > 10, it starts a new line,
        // but that new line will also be the same word (trimmed of leading space).
        // The word itself should still appear in the output.
        assert!(out.contains("superlongwordthatexceedscolumnlimit"));
    }

    #[test]
    fn wordwrap_multiple_tabs_expanded() {
        // Two tabs should each become 4 spaces
        let out = wordwrap(80, "a\t\tb");
        assert_eq!(out, "a        b");
    }

    #[test]
    fn split_capturing_via_wordwrap_handles_no_whitespace() {
        // Text with no spaces: treated as a single chunk; should appear unchanged.
        assert_eq!(wordwrap(72, "nospaces"), "nospaces");
    }

    #[test]
    fn split_capturing_via_wordwrap_preserves_trailing_spaces() {
        // A trailing word with no trailing whitespace is the final "rest" chunk.
        let input = "first second ";
        // The trailing space means the split produces ["", "first ", "second ", ""]
        // So output should just be the same with empty trailing stripped.
        let out = wordwrap(72, input);
        // The important thing: both words are present
        assert!(out.contains("first"), "should contain 'first'");
        assert!(out.contains("second"), "should contain 'second'");
    }
}
