//! The "Export for AI" reports: the scanned tree as a text file an AI
//! assistant can read, with a localized instruction prompt on top.

use crate::i18n::tr_plain;
use crate::safety::{Os, Rules};
use crate::tree::{fmt_size, Kind, Tree};

/// Entries kept by the "only large files" report, to stay under the
/// context limits of AI chats.
const TOP_LARGE: usize = 1000;

/// The instruction on the first line of every report, in the UI language.
/// Files get the stored translation, not the display reordered one
/// ([`tr_plain`]): an AI reads logical order.
pub fn prompt() -> &'static str {
    tr_plain("Hello, my storage is full. Please analyze the attached file list and explain where the space is going. Identify large files/folders and suggest what can be safely deleted to free up space, presenting the information clearly.")
}

/// The deletion guard rails of the scanned target.
fn rules(t: &Tree) -> Rules {
    match &t.source {
        crate::tree::Source::Local => Rules::current(),
        _ => Rules::for_os(Os::Linux, None),
    }
}

/// Live file and folder entries, largest first. Places the safety rules
/// forbid deleting (C:\Windows, pagefile.sys, System Volume Information…)
/// are left out: the report only lists what the user could act on. A
/// forbidden node still has its children visited: "essential folders" are
/// protected themselves while their content can go (with caution).
fn entries(t: &Tree) -> Vec<(u64, String)> {
    let rules = rules(t);
    let mut out = Vec::new();
    let mut stack: Vec<u32> = t.children(t.root).collect();
    while let Some(id) = stack.pop() {
        if !t.is_alive(id) {
            continue;
        }
        stack.extend(t.children(id));
        let n = t.get(id);
        // aggregates (smaller files, hidden space) and mounts have no real path
        if !matches!(n.kind, Kind::Dir | Kind::File) {
            continue;
        }
        let mut path = t.path(id);
        if rules.check(&path).is_forbidden() {
            continue;
        }
        if n.kind == Kind::Dir && !path.ends_with(t.sep) {
            path.push(t.sep);
        }
        out.push((n.size, path));
    }
    out.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    out
}

/// The report text: the localized prompt, a blank line, then
/// `size<TAB>path` lines, largest first.
pub fn report(t: &Tree, large_only: bool) -> String {
    let mut entries = entries(t);
    if large_only {
        entries.truncate(TOP_LARGE);
    }
    let mut s = String::with_capacity(256 + entries.len() * 96);
    s.push_str(prompt());
    s.push_str("\n\n");
    for (size, path) in &entries {
        s.push_str(&fmt_size(*size));
        s.push('\t');
        s.push_str(path);
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Source;

    #[test]
    fn prompt_first_then_rows_largest_first() {
        let mut t = Tree::new("/r", "r", Source::Ssh { host: "h".into() });
        t.add_child_sized(0, "big", Kind::File, 2000, 1);
        let d = t.add_child_sized(0, "dir", Kind::Dir, 0, 0);
        t.add_child_sized(d, "inner", Kind::File, 10, 1);
        let r = report(&t, false);
        let mut lines = r.lines();
        assert!(lines.next().unwrap().starts_with("Hello, my storage is full"));
        assert!(lines.next().unwrap().is_empty());
        assert_eq!(lines.next().unwrap(), "2.0 KB\t/r/big");
        // a folder keeps its path, marked by a trailing separator
        assert!(r.contains("/r/dir/"));
    }

    #[test]
    fn large_only_keeps_a_thousand() {
        let mut t = Tree::new("/r", "r", Source::Ssh { host: "h".into() });
        for i in 0..1200u64 {
            t.add_child_sized(0, &format!("f{i}"), Kind::File, i + 1, 1);
        }
        let r = report(&t, true);
        assert_eq!(r.lines().count(), 2 + TOP_LARGE);
        assert!(r.lines().nth(2).unwrap().ends_with("f1199"));
        assert!(!r.contains("f199"));
    }

    #[test]
    #[cfg(windows)]
    fn filters_windows_system_paths() {
        let mut t = Tree::new("C:\\", "C:", Source::Local);
        t.add_child_sized(0, "pagefile.sys", Kind::File, 8000, 1);
        t.add_child_sized(0, "hiberfil.sys", Kind::File, 4000, 1);
        let w = t.add_child_sized(0, "Windows", Kind::Dir, 0, 0);
        t.add_child_sized(w, "system32", Kind::Dir, 9000, 9);
        t.add_child_sized(0, "System Volume Information", Kind::Dir, 300, 1);
        t.add_child_sized(0, "Users", Kind::Dir, 0, 0);
        let r = report(&t, false);
        assert!(!r.contains("Windows") && !r.contains("pagefile") && !r.contains("hiberfil") && !r.contains("Volume Information") && !r.contains("Users"));
    }
}
