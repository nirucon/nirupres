//! Inline Markdown emphasis (`**bold**`, `*italic*`, `***both***` and the
//! underscore forms), parsed exactly like the 2.x regular expressions.

/// A run of text with its emphasis.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

/// One match of the emphasis pattern at `i`: (end, inner start, inner end, kind).
/// kind: 0 = bold+italic, 1 = bold, 2 = italic.
fn match_at(c: &[char], i: usize) -> Option<(usize, usize, usize, u8)> {
    let n = c.len();
    let at = |k: usize| c.get(k).copied();
    for m in ['*', '_'] {
        // ***x*** / ___x___ : content without the marker or a newline.
        if at(i) == Some(m) && at(i + 1) == Some(m) && at(i + 2) == Some(m) {
            let mut j = i + 3;
            while j < n && c[j] != m && c[j] != '\n' {
                j += 1;
            }
            if j > i + 3 && at(j) == Some(m) && at(j + 1) == Some(m) && at(j + 2) == Some(m) {
                return Some((j + 3, i + 3, j, 0));
            }
        }
    }
    for m in ['*', '_'] {
        // **x** / __x__ : lazy, any char but a newline.
        if at(i) == Some(m) && at(i + 1) == Some(m) {
            let mut j = i + 3;
            while j < n {
                if c[j - 1] == '\n' {
                    break;
                }
                if c[j] == m && at(j + 1) == Some(m) {
                    return Some((j + 2, i + 2, j, 1));
                }
                j += 1;
            }
        }
    }
    for m in ['*', '_'] {
        // (?<!m)m([^m\n]+?)m(?!m)
        if at(i) == Some(m) && (i == 0 || c[i - 1] != m) {
            let mut j = i + 1;
            while j < n && c[j] != m && c[j] != '\n' {
                j += 1;
            }
            if j > i + 1 && at(j) == Some(m) && at(j + 1) != Some(m) {
                return Some((j + 1, i + 1, j, 2));
            }
        }
    }
    None
}

fn runs_into(c: &[char], bold: bool, italic: bool, out: &mut Vec<Run>) {
    let mut pos = 0;
    let mut i = 0;
    let push = |out: &mut Vec<Run>, s: &[char], b: bool, it: bool| {
        if !s.is_empty() {
            out.push(Run {
                text: s.iter().collect(),
                bold: b,
                italic: it,
            });
        }
    };
    while i < c.len() {
        if let Some((end, a, b, kind)) = match_at(c, i) {
            push(out, &c[pos..i], bold, italic);
            let inner = &c[a..b];
            match kind {
                0 => runs_into(inner, true, true, out),
                1 => runs_into(inner, true, italic, out),
                _ => runs_into(inner, bold, true, out),
            }
            pos = end;
            i = end;
        } else {
            i += 1;
        }
    }
    push(out, &c[pos..], bold, italic);
}

/// `markup_runs`: split text into emphasis runs.
pub fn runs(text: &str) -> Vec<Run> {
    let c: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    runs_into(&c, false, false, &mut out);
    out
}

/// True when the text contains emphasis at all (the 2.x fast path test).
pub fn has_markup(text: &str) -> bool {
    let c: Vec<char> = text.chars().collect();
    (0..c.len()).any(|i| match_at(&c, i).is_some())
}

/// `_plain_markup`: remove emphasis markers that are not escaped.
pub fn plain(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if (ch == '*' || ch == '_') && (i == 0 || c[i - 1] != '\\') {
            // `\*\*|__|\*|_`: a doubled marker counts as one match.
            if c.get(i + 1) == Some(&ch) {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// `_format_markdown_selection`: toggle emphasis on each selected line.
pub fn format_selection(text: &str, kind: &str) -> String {
    let text = text.replace('\u{2029}', "\n");
    if kind == "normal" {
        return plain(&text);
    }
    let marker = if kind == "bold" { "**" } else { "*" };
    let mut out = Vec::new();
    for line in text.split('\n') {
        if line.trim().is_empty() {
            out.push(line.to_string());
            continue;
        }
        let lead_len = line.len() - line.trim_start().len();
        let leading = &line[..lead_len];
        let trailing = &line[line.trim_end().len()..];
        let core = line.trim();
        let core = if kind == "bold"
            && core.starts_with("**")
            && core.ends_with("**")
            && core.chars().count() >= 4
        {
            core[2..core.len() - 2].to_string()
        } else if kind == "italic"
            && core.starts_with('*')
            && core.ends_with('*')
            && !core.starts_with("**")
            && !core.ends_with("**")
            && core.chars().count() >= 2
        {
            core[1..core.len() - 1].to_string()
        } else {
            format!("{marker}{core}{marker}")
        };
        out.push(format!("{leading}{core}{trailing}"));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(t: &str, b: bool, i: bool) -> Run {
        Run {
            text: t.into(),
            bold: b,
            italic: i,
        }
    }

    #[test]
    fn emphasis_runs() {
        assert_eq!(
            runs("a **b** *c* ***d***"),
            vec![
                r("a ", false, false),
                r("b", true, false),
                r(" ", false, false),
                r("c", false, true),
                r(" ", false, false),
                r("d", true, true)
            ]
        );
        assert_eq!(runs("x * y"), vec![r("x * y", false, false)]);
        assert_eq!(runs("snake_case_name"), vec![r("snake", false, false), r("case", false, true), r("name", false, false)]);
        assert_eq!(runs("**a *b* c**"), vec![r("a ", true, false), r("b", true, true), r(" c", true, false)]);
        assert!(!has_markup("plain text"));
    }

    #[test]
    fn plain_and_format() {
        assert_eq!(plain("**a** _b_ \\*c"), "a b \\*c");
        assert_eq!(format_selection("one\n  two ", "bold"), "**one**\n  **two** ");
        assert_eq!(format_selection("**one**", "bold"), "one");
        assert_eq!(format_selection("*x*", "italic"), "x");
        assert_eq!(format_selection("**x**", "normal"), "x");
    }
}
