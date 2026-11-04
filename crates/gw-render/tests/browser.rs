use gw_parse::parse_graph_line;
use gw_render::browser::{App, FileEntry, Filters, view};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    style::Color,
};

fn app(fixture: &str, w: u16, h: u16) -> App {
    let p = format!("{}/../gw-parse/tests/fixtures/{fixture}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(p).unwrap();
    let mut a = App::new(Filters::new(vec![]), "main".into(), vec!["origin".into()], 1_793_751_908);
    a.resize(w, h);
    a.push_rows(text.lines().map(parse_graph_line).collect());
    a.finish_load();
    let sha = a.selected_hash().unwrap().to_string();
    a.set_files(
        sha,
        vec![
            FileEntry { code: 'M', path: "src/main.rs".into(), orig: None, stat: Some(Some((12, 3))) },
            FileEntry { code: 'A', path: "docs/very/long/path/to/some/file.md".into(), orig: None, stat: Some(Some((40, 0))) },
            FileEntry { code: 'R', path: "new.rs".into(), orig: Some("old.rs".into()), stat: Some(Some((0, 0))) },
            FileEntry { code: 'M', path: "logo.png".into(), orig: None, stat: Some(None) },
        ],
    );
    a
}

fn key(a: &mut App, c: KeyCode) {
    a.key(KeyEvent::new(c, KeyModifiers::NONE));
}

fn render(a: &App) -> Terminal<TestBackend> {
    let mut t = Terminal::new(TestBackend::new(a.width, a.height)).unwrap();
    t.draw(|f| view::draw(f, a)).unwrap();
    t
}

fn screen(a: &App) -> String {
    render(a).backend().to_string()
}

fn last_line(a: &App) -> String {
    let t = render(a);
    let b = t.backend().buffer();
    (0..b.area.width).map(|x| b[(x, b.area.height - 1)].symbol().to_string()).collect::<String>().trim_end().to_string()
}

#[test]
fn list_linear_wide() {
    insta::assert_snapshot!(screen(&app("log-linear.txt", 120, 12)));
}

#[test]
fn list_merge_graph() {
    let a = app("log-merge.txt", 120, 16);
    let t = render(&a);
    let b = t.backend().buffer();
    // Lane colors: column 0 (lane 0) and column 2 (lane 1) differ on the `| *` row.
    let row = 4; // header(1) + border(1) + rows 0..
    assert_ne!(b[(1, row)].style().fg, b[(3, row)].style().fg, "each lane has its own color");
    insta::assert_snapshot!(t.backend().to_string());
}

#[test]
fn escape_in_subject_is_inert() {
    let a = app("log-all.txt", 140, 30);
    let s = screen(&a);
    assert!(!s.contains('\x1b'));
    assert!(s.contains("side: \\x1b[2J two"), "{s}");
}

#[test]
fn narrow_sidebar_hidden_until_tab() {
    let mut a = app("log-linear.txt", 80, 10);
    let s = screen(&a);
    assert!(!s.contains("Files ("), "{s}");
    key(&mut a, KeyCode::Tab);
    insta::assert_snapshot!("sidebar_80_cols", screen(&a));
}

#[test]
fn diff_pane() {
    let mut a = app("log-linear.txt", 80, 10);
    let sha = a.selected_hash().unwrap().to_string();
    a.set_diff(sha, "src/main.rs".into(), ["diff --git a/x b/x", "@@ -1,2 +1,2 @@", " keep", "-old\tline", "+new line \x1b[31m"].map(String::from).to_vec());
    let t = render(&a);
    let b = t.backend().buffer();
    let find = |needle: &str| (0..b.area.height).find(|&y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>().contains(needle)).unwrap();
    assert_eq!(b[(1, find("+new"))].style().fg, Some(Color::Green));
    assert_eq!(b[(1, find("-old"))].style().fg, Some(Color::Red));
    insta::assert_snapshot!(t.backend().to_string());
}

#[test]
fn hotkeys_per_context() {
    let mut a = app("log-linear.txt", 120, 12);
    let mut out = vec![format!("list:    {}", last_line(&a))];
    key(&mut a, KeyCode::Tab);
    out.push(format!("sidebar: {}", last_line(&a)));
    let sha = a.selected_hash().unwrap().to_string();
    a.set_diff(sha, "x".into(), vec!["+x".into()]);
    out.push(format!("diff:    {}", last_line(&a)));
    key(&mut a, KeyCode::Esc);
    key(&mut a, KeyCode::Char(' '));
    out.push(format!("actions: {}", last_line(&a)));
    key(&mut a, KeyCode::Esc);
    key(&mut a, KeyCode::Char('d'));
    out.push(format!("date:    {}", last_line(&a)));
    let mut narrow = app("log-linear.txt", 40, 12);
    narrow.resize(40, 12);
    out.push(format!("narrow:  {}", last_line(&narrow)));
    insta::assert_snapshot!(out.join("\n"));
}

#[test]
fn branch_modal_and_action_panel() {
    let mut a = app("log-merge.txt", 100, 20);
    a.open_branches(vec!["main".into(), "feat".into(), "release/1.2".into(), "origin/main".into()]);
    for c in "ma".chars() {
        key(&mut a, KeyCode::Char(c));
    }
    let modal = screen(&a);
    key(&mut a, KeyCode::Esc);
    key(&mut a, KeyCode::Char(' '));
    let panel = screen(&a);
    insta::assert_snapshot!(format!("{modal}\n{panel}"));
}
