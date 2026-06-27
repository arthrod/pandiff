//! Faithful port of `node-htmldiff` (htmldiff.js v0.9.4).
//!
//! Compares two HTML documents by tokenizing both, finding matching blocks,
//! deriving insert/delete/replace/equal operations, and re-rendering the
//! combined document with `<ins>`/`<del>` wrappers. Behaviour (including a few
//! quirks of the original implementation) is preserved so output matches the
//! JavaScript library byte-for-byte.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Character / token predicates
// ---------------------------------------------------------------------------

/// Matches the JavaScript `\s` test for a single character closely enough for
/// the HTML produced by Pandoc (ASCII whitespace + NBSP + BOM).
fn is_space_char(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// JS `/[\w\d\#@]/` — note `\w` in JS is ASCII-only.
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '#' || c == '@'
}

static RE_IS_TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*<([^!>][^>]*)>\s*$").unwrap());
static RE_VOID_TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*<[^>]+/>\s*$").unwrap());
static RE_IMG_WRAP: Lazy<Regex> = Lazy::new(|| Regex::new(r"^<img[\s>]").unwrap());
static RE_START_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"^<!--").unwrap());
static RE_END_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"-->$").unwrap());

static RE_KEY_IMG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^<img.*src=['"]([^"']*)['"].*>$"#).unwrap());
static RE_KEY_A: Lazy<Regex> = Lazy::new(|| Regex::new(r#"^<a.*href=['"]([^"']*)['"]"#).unwrap());
static RE_KEY_OBJECT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^<object.*data=['"]([^"']*)['"]"#).unwrap());
static RE_KEY_SMV: Lazy<Regex> = Lazy::new(|| Regex::new(r"^<(svg|math|video)[\s>]").unwrap());
static RE_KEY_IFRAME: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^<iframe.*src=['"]([^"']*)['"].*>"#).unwrap());
static RE_KEY_TAGNAME: Lazy<Regex> = Lazy::new(|| Regex::new(r"<([^\s>]+)[\s>]").unwrap());
static RE_KEY_WS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\s+|&nbsp;|&#160;)").unwrap());

/// Returns the tag name (first word inside `<...>`) or `None` if not a tag.
fn is_tag(token: &str) -> Option<String> {
    let caps = RE_IS_TAG.captures(token)?;
    let inner = caps.get(1)?.as_str().trim();
    let name = inner.split(' ').next().unwrap_or("");
    Some(name.to_string())
}

fn is_void_tag(token: &str) -> bool {
    RE_VOID_TAG.is_match(token)
}

fn is_wrappable(token: &str, atomic: &Regex) -> bool {
    RE_IMG_WRAP.is_match(token)
        || is_tag(token).is_none()
        || start_of_atomic_tag(token, atomic).is_some()
        || is_void_tag(token)
}

fn start_of_atomic_tag(word: &str, atomic: &Regex) -> Option<String> {
    atomic
        .captures(word)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

fn end_of_atomic_tag(word: &str, tag: &str) -> bool {
    // word.substring(word.length - tag.length - 2) === '</' + tag
    let needle = format!("</{tag}");
    let want = tag.len() + 2;
    if word.len() < want {
        return false;
    }
    word[word.len() - want..] == needle
}

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Token {
    string: String,
    key: String,
}

fn create_token(word: &str) -> Token {
    Token {
        string: word.to_string(),
        key: key_for_token(word),
    }
}

fn key_for_token(token: &str) -> String {
    if let Some(c) = RE_KEY_IMG.captures(token) {
        return format!("<img src=\"{}\">", &c[1]);
    }
    if let Some(c) = RE_KEY_A.captures(token) {
        return format!("<a href=\"{}\"></a>", &c[1]);
    }
    if let Some(c) = RE_KEY_OBJECT.captures(token) {
        return format!("<object src=\"{}\"></object>", &c[1]);
    }
    if RE_KEY_SMV.is_match(token) {
        if let Some(uuid) = token.find("data-uuid=\"") {
            let start = &token[..uuid];
            // JS slices 44 UTF-16 units past the index; tags here are ASCII.
            let end = token.get(uuid + 44..).unwrap_or("");
            return format!("{start}{end}");
        }
        return token.to_string();
    }
    if let Some(c) = RE_KEY_IFRAME.captures(token) {
        return format!("<iframe src=\"{}\"></iframe>", &c[1]);
    }
    if let Some(c) = RE_KEY_TAGNAME.captures(token) {
        return format!("<{}>", c[1].to_lowercase());
    }
    if !token.is_empty() {
        return RE_KEY_WS.replace_all(token, " ").into_owned();
    }
    token.to_string()
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum Mode {
    Char,
    Tag,
    Whitespace,
    AtomicTag,
    HtmlComment,
}

fn html_to_tokens(html: &str, atomic: &Regex) -> Vec<Token> {
    let mut mode = Mode::Char;
    let mut current_word = String::new();
    let mut current_atomic_tag = String::new();
    let mut words: Vec<Token> = Vec::new();

    for ch in html.chars() {
        match mode {
            Mode::Tag => {
                if let Some(tag) = start_of_atomic_tag(&current_word, atomic) {
                    mode = Mode::AtomicTag;
                    current_atomic_tag = tag;
                    current_word.push(ch);
                } else if RE_START_COMMENT.is_match(&current_word) {
                    mode = Mode::HtmlComment;
                    current_word.push(ch);
                } else if ch == '>' {
                    current_word.push('>');
                    words.push(create_token(&current_word));
                    current_word.clear();
                    // (JS checks isWhitespace('>') here; always false.)
                    mode = Mode::Char;
                } else {
                    current_word.push(ch);
                }
            }
            Mode::AtomicTag => {
                if ch == '>' && end_of_atomic_tag(&current_word, &current_atomic_tag) {
                    current_word.push('>');
                    words.push(create_token(&current_word));
                    current_word.clear();
                    current_atomic_tag.clear();
                    mode = Mode::Char;
                } else {
                    current_word.push(ch);
                }
            }
            Mode::HtmlComment => {
                current_word.push(ch);
                if RE_END_COMMENT.is_match(&current_word) {
                    current_word.clear();
                    mode = Mode::Char;
                }
            }
            Mode::Char => {
                if ch == '<' {
                    if !current_word.is_empty() {
                        words.push(create_token(&current_word));
                    }
                    current_word = String::from("<");
                    mode = Mode::Tag;
                } else if is_space_char(ch) {
                    if !current_word.is_empty() {
                        words.push(create_token(&current_word));
                    }
                    current_word = ch.to_string();
                    mode = Mode::Whitespace;
                } else if is_word_char(ch) {
                    current_word.push(ch);
                } else if ch == '&' {
                    if !current_word.is_empty() {
                        words.push(create_token(&current_word));
                    }
                    current_word = ch.to_string();
                } else {
                    current_word.push(ch);
                    words.push(create_token(&current_word));
                    current_word.clear();
                }
            }
            Mode::Whitespace => {
                if ch == '<' {
                    if !current_word.is_empty() {
                        words.push(create_token(&current_word));
                    }
                    current_word = String::from("<");
                    mode = Mode::Tag;
                } else if is_space_char(ch) {
                    current_word.push(ch);
                } else {
                    if !current_word.is_empty() {
                        words.push(create_token(&current_word));
                    }
                    current_word = ch.to_string();
                    mode = Mode::Char;
                }
            }
        }
    }
    if !current_word.is_empty() {
        words.push(create_token(&current_word));
    }
    words
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

type TokenMap = HashMap<String, Vec<usize>>;

fn create_map(tokens: &[Token]) -> TokenMap {
    let mut map: TokenMap = HashMap::new();
    for (index, token) in tokens.iter().enumerate() {
        map.entry(token.key.clone()).or_default().push(index);
    }
    map
}

#[derive(Clone, Debug)]
struct Match {
    start_in_before: usize,
    start_in_after: usize,
    length: usize,
    end_in_before: i64,
    end_in_after: i64,
    segment_start_in_before: usize,
    segment_start_in_after: usize,
    segment_end_in_before: i64,
    segment_end_in_after: i64,
}

impl Match {
    fn new(
        start_in_before: usize,
        start_in_after: usize,
        length: usize,
        before_index: usize,
        after_index: usize,
    ) -> Match {
        let abs_before = start_in_before + before_index;
        let abs_after = start_in_after + after_index;
        Match {
            start_in_before: abs_before,
            start_in_after: abs_after,
            length,
            end_in_before: abs_before as i64 + length as i64 - 1,
            end_in_after: abs_after as i64 + length as i64 - 1,
            segment_start_in_before: start_in_before,
            segment_start_in_after: start_in_after,
            segment_end_in_before: start_in_before as i64 + length as i64 - 1,
            segment_end_in_after: start_in_after as i64 + length as i64 - 1,
        }
    }
}

fn compare_matches(m1: &Match, m2: &Match) -> i32 {
    if m2.end_in_before < m1.start_in_before as i64 && m2.end_in_after < m1.start_in_after as i64 {
        -1
    } else if m2.start_in_before as i64 > m1.end_in_before
        && m2.start_in_after as i64 > m1.end_in_after
    {
        1
    } else {
        0
    }
}

struct BstNode {
    value: Match,
    left: Option<Box<BstNode>>,
    right: Option<Box<BstNode>>,
}

#[derive(Default)]
struct MatchBst {
    root: Option<Box<BstNode>>,
}

impl MatchBst {
    fn add(&mut self, value: Match) {
        let node = Box::new(BstNode {
            value,
            left: None,
            right: None,
        });
        match self.root {
            None => self.root = Some(node),
            Some(ref mut root) => {
                let mut current: &mut Box<BstNode> = root;
                loop {
                    let position = compare_matches(&current.value, &node.value);
                    if position == -1 {
                        if current.left.is_some() {
                            current = current.left.as_mut().unwrap();
                        } else {
                            current.left = Some(node);
                            break;
                        }
                    } else if position == 1 {
                        if current.right.is_some() {
                            current = current.right.as_mut().unwrap();
                        } else {
                            current.right = Some(node);
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    }

    fn into_vec(self) -> Vec<Match> {
        fn in_order(node: Option<Box<BstNode>>, out: &mut Vec<Match>) {
            if let Some(n) = node {
                in_order(n.left, out);
                out.push(n.value);
                in_order(n.right, out);
            }
        }
        let mut out = Vec::new();
        in_order(self.root, &mut out);
        out
    }
}

struct Segment {
    before_tokens: Vec<Token>,
    after_tokens: Vec<Token>,
    after_map: TokenMap,
    before_index: usize,
    after_index: usize,
}

fn create_segment(
    before_tokens: Vec<Token>,
    after_tokens: Vec<Token>,
    before_index: usize,
    after_index: usize,
) -> Segment {
    let after_map = create_map(&after_tokens);
    Segment {
        before_tokens,
        after_tokens,
        after_map,
        before_index,
        after_index,
    }
}

fn find_best_match(segment: &Segment) -> Option<Match> {
    let before_tokens = &segment.before_tokens;
    let after_map = &segment.after_map;
    let mut last_space: Option<usize> = None;
    let mut best_match: Option<Match> = None;

    for before_index in 0..before_tokens.len() {
        let mut look_behind = false;
        let remaining_tokens = before_tokens.len() - before_index;
        if let Some(ref bm) = best_match {
            if remaining_tokens < bm.length {
                break;
            }
        }

        let before_token = &before_tokens[before_index];
        if before_token.key == " " {
            last_space = Some(before_index);
            continue;
        }
        if last_space == Some(before_index.wrapping_sub(1)) && before_index > 0 {
            look_behind = true;
        }

        let after_locations = match after_map.get(&before_token.key) {
            Some(v) => v,
            None => continue,
        };

        for &after_index in after_locations {
            let best_len = best_match.as_ref().map(|m| m.length).unwrap_or(0);
            if let Some(m) =
                get_full_match(segment, before_index, after_index, best_len, look_behind)
            {
                if m.length > best_len {
                    best_match = Some(m);
                }
            }
        }
    }

    best_match
}

fn get_full_match(
    segment: &Segment,
    before_start: usize,
    after_start: usize,
    min_length: usize,
    look_behind: bool,
) -> Option<Match> {
    let before_tokens = &segment.before_tokens;
    let after_tokens = &segment.after_tokens;

    let min_before = before_start + min_length;
    let min_after = after_start + min_length;
    if min_before >= before_tokens.len() || min_after >= after_tokens.len() {
        return None;
    }

    if min_length > 0 && before_tokens[min_before].key != after_tokens[min_after].key {
        return None;
    }

    let mut searching = true;
    let mut current_length = 1usize;
    let mut before_index = before_start + current_length;
    let mut after_index = after_start + current_length;

    while searching && before_index < before_tokens.len() && after_index < after_tokens.len() {
        if before_tokens[before_index].key == after_tokens[after_index].key {
            current_length += 1;
            before_index = before_start + current_length;
            after_index = after_start + current_length;
        } else {
            searching = false;
        }
    }

    let mut before_start = before_start;
    let mut after_start = after_start;
    if look_behind
        && before_start > 0
        && after_start > 0
        && before_tokens[before_start - 1].key == " "
        && after_tokens[after_start - 1].key == " "
    {
        before_start -= 1;
        after_start -= 1;
        current_length += 1;
    }

    Some(Match::new(
        before_start,
        after_start,
        current_length,
        segment.before_index,
        segment.after_index,
    ))
}

fn find_matching_blocks(segment: Segment) -> Vec<Match> {
    let mut matches = MatchBst::default();
    let mut segments = vec![segment];

    while let Some(segment) = segments.pop() {
        if let Some(m) = find_best_match(&segment) {
            if m.length > 0 {
                if m.segment_start_in_before > 0 && m.segment_start_in_after > 0 {
                    let left_before = segment.before_tokens[..m.segment_start_in_before].to_vec();
                    let left_after = segment.after_tokens[..m.segment_start_in_after].to_vec();
                    segments.push(create_segment(
                        left_before,
                        left_after,
                        segment.before_index,
                        segment.after_index,
                    ));
                }

                let right_before_start = (m.segment_end_in_before + 1) as usize;
                let right_after_start = (m.segment_end_in_after + 1) as usize;
                let right_before = segment.before_tokens[right_before_start..].to_vec();
                let right_after = segment.after_tokens[right_after_start..].to_vec();
                let right_before_index = segment.before_index + right_before_start;
                let right_after_index = segment.after_index + right_after_start;

                if !right_before.is_empty() && !right_after.is_empty() {
                    segments.push(create_segment(
                        right_before,
                        right_after,
                        right_before_index,
                        right_after_index,
                    ));
                }

                matches.add(m);
            }
        }
    }

    matches.into_vec()
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Action {
    Equal,
    Insert,
    Delete,
    Replace,
}

#[derive(Clone, Debug)]
struct Operation {
    action: Action,
    start_in_before: usize,
    end_in_before: i64,
    start_in_after: usize,
    end_in_after: i64,
}

fn calculate_operations(before_tokens: &[Token], after_tokens: &[Token]) -> Vec<Operation> {
    let mut position_in_before = 0usize;
    let mut position_in_after = 0usize;
    let mut operations: Vec<Operation> = Vec::new();

    let segment = create_segment(before_tokens.to_vec(), after_tokens.to_vec(), 0, 0);
    let mut matches = find_matching_blocks(segment);
    matches.push(Match::new(before_tokens.len(), after_tokens.len(), 0, 0, 0));

    for m in &matches {
        let mut action: Option<Action> = None;
        if position_in_before == m.start_in_before {
            if position_in_after != m.start_in_after {
                action = Some(Action::Insert);
            }
        } else {
            action = Some(Action::Delete);
            if position_in_after != m.start_in_after {
                action = Some(Action::Replace);
            }
        }
        if let Some(act) = action {
            operations.push(Operation {
                action: act,
                start_in_before: position_in_before,
                end_in_before: if act != Action::Insert {
                    m.start_in_before as i64 - 1
                } else {
                    -1
                },
                start_in_after: position_in_after,
                end_in_after: if act != Action::Delete {
                    m.start_in_after as i64 - 1
                } else {
                    -1
                },
            });
        }
        if m.length != 0 {
            operations.push(Operation {
                action: Action::Equal,
                start_in_before: m.start_in_before,
                end_in_before: m.end_in_before,
                start_in_after: m.start_in_after,
                end_in_after: m.end_in_after,
            });
        }
        position_in_before = (m.end_in_before + 1) as usize;
        position_in_after = (m.end_in_after + 1) as usize;
    }

    // Post-process: merge consecutive replace operations.
    //
    // The original also merges a "single whitespace equal" that follows a
    // replace, but its `isSingleWhitespace` check coerces an array of token
    // *objects* to the string "[object Object]", so it is always false. We
    // preserve that behaviour (the branch is dead).
    let mut post_processed: Vec<Operation> = Vec::new();
    let mut last_action = Action::Equal; // sentinel != replace; JS uses {action:'none'}
    let mut has_last = false;
    for op in operations {
        if has_last && op.action == Action::Replace && last_action == Action::Replace {
            let last = post_processed.last_mut().unwrap();
            last.end_in_before = op.end_in_before;
            last.end_in_after = op.end_in_after;
        } else {
            last_action = op.action;
            post_processed.push(op);
            has_last = true;
        }
    }
    post_processed
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

static RE_TRAILING_GT: Lazy<Regex> = Lazy::new(|| Regex::new(r">\s*$").unwrap());

struct Note {
    is_wrappable: bool,
    inserted_tag: bool,
}

fn build_notes(tokens: &[Token], atomic: &Regex) -> Vec<Note> {
    let mut notes: Vec<Note> = Vec::new();
    let mut tag_stack: Vec<(String, usize)> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        notes.push(Note {
            is_wrappable: is_wrappable(&token.string, atomic),
            inserted_tag: false,
        });
        let tag = if !is_void_tag(&token.string) {
            is_tag(&token.string)
        } else {
            None
        };
        if let Some(tag) = tag {
            if let Some((last_tag, last_pos)) = tag_stack.last().cloned() {
                if format!("/{last_tag}") == tag {
                    notes[last_pos].inserted_tag = true;
                    tag_stack.pop();
                    continue;
                }
            }
            tag_stack.push((tag, index));
        }
    }
    notes
}

/// Combine tokens into wrapped HTML, grouping by wrappability.
fn combine_wrap(
    tokens: &[Token],
    notes: &[Note],
    atomic: &Regex,
    tag: &str,
    attrs: &str,
    data_prefix: &str,
    op_index: usize,
) -> String {
    let _ = atomic;
    // Apply tagFn to tokens whose opening tag is closed within this set.
    let mut toks: Vec<String> = tokens.iter().map(|t| t.string.clone()).collect();
    for (i, note) in notes.iter().enumerate() {
        if note.inserted_tag {
            let prefix = if data_prefix.is_empty() {
                String::new()
            } else {
                format!("{data_prefix}-")
            };
            let data_attrs =
                format!(" data-diff-node=\"{tag}\" data-{prefix}operation-index=\"{op_index}\"");
            toks[i] = RE_TRAILING_GT
                .replace(&toks[i], |c: &regex::Captures| {
                    format!("{}{}", data_attrs, &c[0])
                })
                .into_owned();
        }
    }

    // Build segments of contiguous same-wrappability tokens.
    struct Seg {
        is_wrappable: bool,
        tokens: Vec<String>,
    }
    let mut list: Vec<Seg> = Vec::new();
    let mut status: Option<bool> = None;
    let mut last_index = 0usize;
    for index in 0..toks.len() {
        if status.is_none() {
            status = Some(notes[index].is_wrappable);
        }
        let st = notes[index].is_wrappable;
        if Some(st) != status {
            list.push(Seg {
                is_wrappable: status.unwrap(),
                tokens: toks[last_index..index].to_vec(),
            });
            last_index = index;
            status = Some(st);
        }
        if index == toks.len() - 1 {
            list.push(Seg {
                is_wrappable: status.unwrap(),
                tokens: toks[last_index..index + 1].to_vec(),
            });
        }
    }

    let mut out = String::new();
    for seg in &list {
        if seg.is_wrappable {
            let val = seg.tokens.concat();
            if !val.trim().is_empty() {
                out.push_str(&format!("<{tag}{attrs}>{val}</{tag}>"));
            }
        } else {
            out.push_str(&seg.tokens.concat());
        }
    }
    out
}

fn wrap(
    tag: &str,
    content: &[Token],
    op_index: usize,
    data_prefix: &str,
    class_name: Option<&str>,
    atomic: &Regex,
) -> String {
    let prefix = if data_prefix.is_empty() {
        String::new()
    } else {
        format!("{data_prefix}-")
    };
    let mut attrs = format!(" data-{prefix}operation-index=\"{op_index}\"");
    if let Some(cn) = class_name {
        attrs.push_str(&format!(" class=\"{cn}\""));
    }
    let notes = build_notes(content, atomic);
    combine_wrap(content, &notes, atomic, tag, &attrs, data_prefix, op_index)
}

#[allow(clippy::too_many_arguments)]
fn render_operations(
    before_tokens: &[Token],
    after_tokens: &[Token],
    operations: &[Operation],
    data_prefix: &str,
    class_name: Option<&str>,
    atomic: &Regex,
) -> String {
    let mut out = String::new();
    for (index, op) in operations.iter().enumerate() {
        match op.action {
            Action::Equal => {
                let toks = &after_tokens[op.start_in_after..(op.end_in_after + 1) as usize];
                for t in toks {
                    out.push_str(&t.string);
                }
            }
            Action::Insert => {
                let toks = after_tokens[op.start_in_after..(op.end_in_after + 1) as usize].to_vec();
                out.push_str(&wrap("ins", &toks, index, data_prefix, class_name, atomic));
            }
            Action::Delete => {
                let toks =
                    before_tokens[op.start_in_before..(op.end_in_before + 1) as usize].to_vec();
                out.push_str(&wrap("del", &toks, index, data_prefix, class_name, atomic));
            }
            Action::Replace => {
                let del =
                    before_tokens[op.start_in_before..(op.end_in_before + 1) as usize].to_vec();
                let ins = after_tokens[op.start_in_after..(op.end_in_after + 1) as usize].to_vec();
                out.push_str(&wrap("del", &del, index, data_prefix, class_name, atomic));
                out.push_str(&wrap("ins", &ins, index, data_prefix, class_name, atomic));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

const DEFAULT_ATOMIC: &str = r"^<(iframe|object|math|svg|script|video|head|style|a)";

/// Compares two HTML strings and returns the combined content with differences
/// wrapped in `<ins>`/`<del>` tags. Mirrors `node-htmldiff`'s default call.
pub fn htmldiff(before: &str, after: &str) -> String {
    htmldiff_opts(before, after, None, "", None)
}

/// Full-featured variant matching the JS `diff(before, after, className,
/// dataPrefix, atomicTags)` signature.
pub fn htmldiff_opts(
    before: &str,
    after: &str,
    class_name: Option<&str>,
    data_prefix: &str,
    atomic_tags: Option<&str>,
) -> String {
    if before == after {
        return before.to_string();
    }
    let atomic = match atomic_tags {
        Some(list) => {
            let cleaned = list.replace(char::is_whitespace, "").replace(',', "|");
            Regex::new(&format!("^<({cleaned})")).unwrap()
        }
        None => Regex::new(DEFAULT_ATOMIC).unwrap(),
    };
    let before_tokens = html_to_tokens(before, &atomic);
    let after_tokens = html_to_tokens(after, &atomic);
    let ops = calculate_operations(&before_tokens, &after_tokens);
    render_operations(
        &before_tokens,
        &after_tokens,
        &ops,
        data_prefix,
        class_name,
        &atomic,
    )
}

#[cfg(test)]
mod tests;
