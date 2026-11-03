use gw_parse::*;

fn fx(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn events(name: &str) -> Vec<Event> {
    let mut s = Splitter::default();
    let mut lines = s.push(&fx(name));
    lines.extend(s.finish());
    lines.iter().map(|l| classify(l)).collect()
}

#[test]
fn classify_push_ff() {
    insta::assert_debug_snapshot!(events("push-ff.stderr"));
}

#[test]
fn classify_push_new() {
    insta::assert_debug_snapshot!(events("push-new.stderr"));
}

#[test]
fn classify_clone() {
    // Only the distinct (label, pct) transitions and non-progress lines, to keep the snapshot readable.
    let mut out: Vec<Event> = Vec::new();
    for e in events("clone.stderr") {
        if let (Event::Progress(p), Some(Event::Progress(last))) = (&e, out.last()) {
            if p.label == last.label && p.tail.is_empty() && p.pct.map(|x| x % 25 != 0).unwrap_or(false) {
                continue;
            }
        }
        out.push(e);
    }
    insta::assert_debug_snapshot!(out);
}

#[test]
fn push_porcelain() {
    let all: Vec<_> = ["push-ff", "push-new", "push-rejected", "push-uptodate"]
        .iter()
        .map(|n| (n.to_string(), parse_push_porcelain(&String::from_utf8(fx(&format!("{n}.stdout"))).unwrap())))
        .collect();
    insta::assert_debug_snapshot!(all);
}

#[test]
fn push_porcelain_forced_and_deleted() {
    insta::assert_debug_snapshot!(parse_push_porcelain(
        "To x\n+\trefs/heads/a:refs/heads/a\t+1111111...2222222 (forced update)\n-\t:refs/heads/old\t[deleted]\nDone\n"
    ));
}

#[test]
fn fetch_porcelain() {
    insta::assert_debug_snapshot!(parse_fetch_porcelain(&String::from_utf8(fx("fetch.stdout")).unwrap()));
}

#[test]
fn human_ref_lines() {
    let mut s = Splitter::default();
    let refs: Vec<_> = s.push(&fx("pull-ff.stderr")).iter().map(|l| classify(l)).filter(|e| !matches!(e, Event::Progress(_))).collect();
    let extra: Vec<_> = [
        " * [new branch]      feat       -> origin/feat",
        " + 1111111...2222222 force      -> origin/force  (forced update)",
        " ! [rejected]        main       -> main  (non-fast-forward)",
        " - [deleted]         (none)     -> origin/gone",
        " = [up to date]      main       -> origin/main",
    ]
    .iter()
    .map(|l| parse_human_ref_line(l))
    .collect();
    insta::assert_debug_snapshot!((refs, extra));
}

#[test]
fn status_fixtures() {
    for n in ["status-mixed", "status-clean", "status-detached", "status-conflict"] {
        insta::assert_debug_snapshot!(n, parse_status_v2(&fx(&format!("{n}.z"))));
    }
}
