use gw_parse::*;
use gw_render::{refs::*, remote_box::render_box, status::{LineStats, render_status}, style::visible_width};

fn fx(name: &str) -> String {
    let p = format!("{}/../gw-parse/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    String::from_utf8_lossy(&std::fs::read(p).unwrap()).into_owned()
}

/// Makes escapes readable in snapshots.
fn show(s: &str) -> String {
    s.replace('\x1b', "␛")
}

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn remote_lines(name: &str) -> Vec<String> {
    let mut s = Splitter::default();
    s.push(fx(name).as_bytes())
        .iter()
        .filter_map(|l| match classify(l) {
            Event::Remote(r) => Some(r),
            _ => None,
        })
        .collect()
}

fn assert_aligned(b: &str) {
    let widths: Vec<usize> = b.lines().map(visible_width).collect();
    assert!(widths.windows(2).all(|w| w[0] == w[1]), "unequal widths {widths:?}\n{b}");
}

#[test]
fn box_gitlab() {
    let b = render_box(&remote_lines("push-new.stderr"), &lines(&["feat-forms-constructor"]), 100, true);
    assert_aligned(&b);
    assert_eq!(b.lines().count(), 4, "two content lines + borders");
    insta::assert_snapshot!(show(&b));
}

#[test]
fn box_github() {
    let l = lines(&[
        "",
        "Create a pull request for 'feat' on GitHub by visiting:",
        "     https://github.com/o/r/pull/new/feat",
        "",
    ]);
    let b = render_box(&l, &lines(&["feat"]), 100, false);
    assert_aligned(&b);
    insta::assert_snapshot!(show(&b));
}

#[test]
fn box_generic() {
    let b = render_box(&lines(&["Processing changes: refs: 1, done"]), &[], 100, false);
    assert_aligned(&b);
    insta::assert_snapshot!(show(&b));
}

#[test]
fn box_long_url_80_cols() {
    let url = format!("https://gitlab.example.com/group/sub/repo/-/merge_requests/new?{}", "x".repeat(120 - 63));
    assert_eq!(url.len(), 120);
    let b = render_box(&lines(&["View merge request:", &format!("  {url}")]), &[], 80, false);
    assert_aligned(&b);
    assert_eq!(visible_width(b.lines().next().unwrap()), 80);
    assert!(b.contains(&format!("\x1b]8;;{url}\x1b\\")), "full link target kept");
    assert!(gw_render::style::strip_escapes(&b).contains("…"));
    insta::assert_snapshot!(show(&b));
}

#[test]
fn untrusted_text_is_inert() {
    let b = render_box(&lines(&["hi \x1b[2Jhttps://x.y/\x07\x1b]8;;evil"]), &[], 80, false);
    assert_aligned(&b);
    assert!(!gw_render::style::strip_escapes(&b).contains('\x1b') && !b.contains('\x07'), "{b:?}");
    let st = parse_status_v2(b"# branch.head m\0? a\x1b[31mb\0");
    let s = render_status(&st, "", &LineStats::default(), false);
    assert!(s.contains("? a\\x1b[31mb") && !s.contains('\x1b'), "{s:?}");
}

#[test]
fn box_empty() {
    assert_eq!(render_box(&lines(&["", ""]), &[], 80, true), "");
}

#[test]
fn ref_lists() {
    let mut out = String::new();
    for n in ["push-ff", "push-new", "push-rejected"] {
        let ev = parse_push_porcelain(&fx(&format!("{n}.stdout")));
        let target = ev.iter().find_map(|e| if let Event::Target(t) = e { Some(t.as_str()) } else { None });
        let refs: Vec<_> = ev.iter().filter_map(|e| if let Event::RefUpdate(r) = e { Some(r.clone()) } else { None }).collect();
        out += &render_refs(target, &refs, true);
    }
    out += &render_refs(None, &parse_fetch_porcelain(&fx("fetch.stdout")), true);
    let mut s = Splitter::default();
    let pull: Vec<_> = s.push(fx("pull-ff.stderr").as_bytes()).iter().filter_map(|l| parse_human_ref_line(l)).collect();
    out += &render_refs(Some("file:///tmp/w/gitlab"), &pull, true);
    out += &render_refs(None, &parse_push_porcelain("+\trefs/heads/a:refs/heads/a\t+1111111...2222222 (forced update)\n-\t:refs/heads/old\t[deleted]\n").into_iter().filter_map(|e| if let Event::RefUpdate(r) = e { Some(r) } else { None }).collect::<Vec<_>>(), true);
    insta::assert_snapshot!(show(&out));
}

#[test]
fn up_to_date() {
    let ev = parse_push_porcelain(&fx("push-uptodate.stdout"));
    let refs: Vec<_> = ev.into_iter().filter_map(|e| if let Event::RefUpdate(r) = e { Some(r) } else { None }).collect();
    assert!(all_up_to_date(&refs));
    assert_eq!(render_up_to_date(true), "\x1b[2mEverything up-to-date\x1b[0m\n");
}

#[test]
fn pull_summary() {
    let l: Vec<String> = fx("pull-ff.stdout").lines().map(str::to_string).collect();
    insta::assert_snapshot!(show(&render_pull_summary(&l, true)));
}

#[test]
fn status_views() {
    let mut out = String::new();
    for (n, prefix) in [("status-mixed", ""), ("status-mixed", "sub/"), ("status-clean", ""), ("status-detached", ""), ("status-conflict", "")] {
        out += &format!("== {n} prefix={prefix:?}\n");
        out += &render_status(&parse_status_v2(fx(&format!("{n}.z")).as_bytes()), prefix, &LineStats::default(), false);
    }
    // numstat: plain, binary, and rename (-z form); untracked counted by caller.
    let staged = parse_numstat_z(b"1\t0\tadded.txt\0-\t-\tf1.txt\x000\t0\t\0f6.txt\0renamed.txt\0");
    let stats = LineStats { staged, unstaged: parse_numstat_z(b"12\t3\tf1.txt\x001\t0\tf2.txt\0"), untracked: [("n.txt".to_string(), Some((1, 0)))].into(), ..Default::default() };
    out += "== status-mixed with stats\n";
    out += &render_status(&parse_status_v2(fx("status-mixed.z").as_bytes()), "", &stats, false);
    assert!(!out.contains("(use \"git"));
    insta::assert_snapshot!(out);
}

#[test]
fn static_views() {
    use gw_render::views::*;
    let branches = parse_branches(&fx("branches.txt"));
    let stashes: Vec<_> = parse_stash_list(&fx("stash-list.txt")).into_iter().zip([(1, 1, 0), (2, 3, 1)]).map(|(s, (f, a, d))| (s, f, a, d)).collect();
    let out = format!(
        "{}\n{}\n{}\n{}",
        render_branches(&branches, 100, false),
        render_commit_summary("a1b2c3d", "main", "fix: x", 2, 10, 3, false),
        render_stash_list(&stashes, false),
        render_stash_list(&[], false)
    );
    assert!(!out.contains('\x1b'), "escape in branch subject is sanitized");
    insta::assert_snapshot!(out);
    let colored = render_branches(&branches, 100, true);
    assert!(colored.contains("\x1b[31mgone"), "gone upstream in red");
}

fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.split('\x1b');
    out += it.next().unwrap_or("");
    for t in it {
        // drop `[...m`
        let end = t.find('m').map_or(0, |i| i + 1);
        out += &t[end..];
    }
    out
}

#[test]
fn pull_summary_all_lines() {
    let mut out = String::new();
    for n in ["pull-submodules.stdout", "pull-merge.stdout", "pull-conflict.stdout", "clone-submodules.stdout"] {
        let l: Vec<String> = fx(n).lines().map(str::to_string).collect();
        let colored = render_pull_summary(&l, true);
        let nc = render_pull_summary(&l, false);
        assert!(!nc.contains('\x1b'), "{n}");
        assert_eq!(plain(&colored), nc, "{n}: color changes only styling");
        // NO_COLOR: git's lines unchanged, except submodule lines which become the section.
        let kept: Vec<&String> = l.iter().filter(|x| !x.starts_with("Submodule ")).collect();
        let nc_lines: Vec<&str> = nc.lines().collect();
        for (a, b) in kept.iter().zip(&nc_lines) {
            assert_eq!(a.as_str(), *b, "{n}");
        }
        out += &format!("== {n}\n{}", show(&colored));
    }
    assert!(out.contains("Submodules"));
    insta::assert_snapshot!(out);
}

#[test]
fn pull_submodule_section_plain() {
    let l: Vec<String> = fx("pull-submodules.stdout").lines().map(str::to_string).collect();
    let nc = render_pull_summary(&l, false);
    assert!(nc.ends_with("Submodules\n  lib  checked out 5d71523\n"), "{nc}");
    assert!(!nc.contains("Submodule path"));
}

#[test]
fn ref_groups_with_submodule() {
    let mut s = Splitter::default();
    let mut groups: Vec<RefGroup> = Vec::new();
    let mut pending = None;
    for l in s.push(fx("pull-submodules.stderr").as_bytes()) {
        match classify(&l) {
            Event::SubmoduleFetch(p) => pending = Some(p),
            Event::Target(t) => groups.push(RefGroup { submodule: pending.take(), target: Some(t), refs: vec![] }),
            Event::RefUpdate(r) => groups.last_mut().unwrap().refs.push(r),
            _ => {}
        }
    }
    groups.push(RefGroup { submodule: Some("doc".into()), target: Some("/tmp/fx/doc".into()), refs: vec![] });
    let out = render_ref_groups(&groups, false);
    assert_eq!(
        out,
        "-> /tmp/fx/app\n  ✓ aae6093..80cba0e  main -> origin/main\n-> submodule lib  /tmp/fx/lib\n  ✓ bcfd07d..5d71523  main -> origin/main\n"
    );
    insta::assert_snapshot!(show(&render_ref_groups(&groups, true)));
}

#[test]
fn status_submodules() {
    let st = parse_status_v2(fx("status-submodules.z").as_bytes());
    let stats = LineStats {
        unstaged: parse_numstat_z(b"1\t0\tx\0"),
        checkouts: parse_submodule_status(&fx("submodule-status.txt")),
        ..Default::default()
    };
    let out = render_status(&st, "", &stats, false);
    assert!(out.contains("Changed (1)\n  M x  +1 -0\n"), "{out}");
    assert!(out.contains("Submodules (2)\n"), "{out}");
    assert!(out.contains("  M doc  bcfd07d → 82aa575  new commits, untracked content\n"), "{out}");
    assert!(out.contains("  M lib  5d71523 → 26bb4d4  staged, modified content\n"), "{out}");
    assert!(!out.contains("Staged"), "{out}");
    assert!(out.ends_with("Total +1 -0\n"), "{out}");
    insta::assert_snapshot!(show(&render_status(&st, "", &stats, true)));

    // Spec scenarios built from porcelain records.
    let one = |rec: &str, co: &[(&str, &str)]| {
        let st = parse_status_v2(format!("# branch.head main\0{rec}\0").as_bytes());
        let stats = LineStats { checkouts: co.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() };
        render_status(&st, "", &stats, false)
    };
    let h = "256f977aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let n = "5a1436fbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let out = one(&format!("1 .M SC.. 160000 160000 160000 {h} {h} lib"), &[("lib", n)]);
    assert!(out.contains("  M lib  256f977 → 5a1436f  new commits\n"), "{out}");
    assert!(!out.contains("Total"), "only a submodule changed: {out}");
    let out = one(&format!("1 .M S.M. 160000 160000 160000 {h} {h} lib"), &[]);
    assert!(out.contains("  M lib  256f977  modified content\n"), "{out}");
    let out = one(&format!("1 M. S... 160000 160000 160000 {h} {n} lib"), &[]);
    assert!(out.contains("  M lib  256f977 → 5a1436f  staged\n"), "{out}");
    // Checkout unknown: recorded commit only.
    let out = one(&format!("1 .M SC.. 160000 160000 160000 {h} {h} lib"), &[]);
    assert!(out.contains("  M lib  256f977  new commits\n"), "{out}");
}
