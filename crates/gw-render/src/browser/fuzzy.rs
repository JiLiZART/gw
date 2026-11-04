//! Small fuzzy matcher: case-insensitive subsequence with bonuses for runs and word starts.

/// Matches every whitespace-separated term of `query` against `fields`.
/// Each term must match some field. Returns the total score and, per field,
/// the sorted char positions that matched (for highlighting).
pub fn score(query: &str, fields: &[&str]) -> Option<(i64, Vec<Vec<usize>>)> {
    let mut total = 0;
    let mut marks = vec![Vec::new(); fields.len()];
    for term in query.split_whitespace() {
        let best = fields
            .iter()
            .enumerate()
            .filter_map(|(i, f)| term_score(term, f).map(|(s, pos)| (s, i, pos)))
            .max_by_key(|(s, i, _)| (*s, std::cmp::Reverse(*i)))?;
        total += best.0;
        marks[best.1].extend(best.2);
    }
    for m in &mut marks {
        m.sort_unstable();
        m.dedup();
    }
    Some((total, marks))
}

/// Greedy subsequence match, preferring a start at a word boundary.
fn term_score(term: &str, field: &str) -> Option<(i64, Vec<usize>)> {
    let hay: Vec<char> = field.chars().flat_map(char::to_lowercase).collect();
    let needle: Vec<char> = term.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    let word_start = |i: usize| i == 0 || !hay[i - 1].is_alphanumeric();
    // Try each possible start of the first char; keep the best scoring alignment.
    let mut best: Option<(i64, Vec<usize>)> = None;
    for start in (0..hay.len()).filter(|&i| hay[i] == needle[0]) {
        let mut pos = vec![start];
        let mut j = start + 1;
        for &c in &needle[1..] {
            while j < hay.len() && hay[j] != c {
                j += 1;
            }
            if j == hay.len() {
                break;
            }
            pos.push(j);
            j += 1;
        }
        if pos.len() < needle.len() {
            break; // later starts cannot match either
        }
        let mut s = 0;
        for (k, &p) in pos.iter().enumerate() {
            s += 1;
            if word_start(p) {
                s += 8;
            }
            if k > 0 && pos[k - 1] + 1 == p {
                s += 5;
            }
        }
        if best.as_ref().is_none_or(|b| s > b.0) {
            best = Some((s, pos));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::score;

    #[test]
    fn hash_prefix_ranks_first() {
        let a = score("a22b", &["a22b7ed", "Nikolay", "feat: x"]).unwrap().0;
        let b = score("a22b", &["0a2c2b1", "Nikolay", "feat: x"]).unwrap().0;
        assert!(a > b);
    }

    #[test]
    fn terms_across_fields() {
        let (_, m) = score("nik fix", &["abc1234", "Nikolay", "fix: typo"]).unwrap();
        assert_eq!(m[1], vec![0, 1, 2]);
        assert_eq!(m[2], vec![0, 1, 2]);
    }

    #[test]
    fn no_match() {
        assert!(score("zzz", &["abc", "Nikolay", "fix"]).is_none());
        assert!(score("nik zzz", &["abc", "Nikolay", "fix"]).is_none(), "every term must match");
    }
}
