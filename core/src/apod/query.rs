pub const MAX_TERMS: usize = 24;
pub const MAX_PREFIX_TERMS: usize = 4;

pub fn fts_query(raw: &str) -> Option<String> {
    let mut budget = Budget::new();
    let rendered: Vec<String> = groups(&tokenize(raw))
        .iter()
        .map(|group| group.truncated(&mut budget))
        .filter_map(|group| group.render())
        .collect();

    match rendered.len() {
        0 => None,
        1 => rendered.into_iter().next(),
        _ => Some(
            rendered
                .iter()
                .map(|group| format!("({group})"))
                .collect::<Vec<_>>()
                .join(" OR "),
        ),
    }
}

#[derive(Debug, Clone)]
struct Term {
    text: String,
    quoted: bool,
    prefix: bool,
    negated: bool,
}

impl Term {
    fn render(&self) -> String {
        let escaped: String = self
            .text
            .chars()
            .filter(|ch| *ch != '"' && !ch.is_control())
            .collect();
        let star = if self.prefix { "*" } else { "" };
        format!("\"{escaped}\"{star}")
    }
}

#[derive(Debug, Default)]
struct Group {
    positive: Vec<Term>,
    negative: Vec<Term>,
}

struct Budget {
    terms: usize,
    prefixes: usize,
}

impl Budget {
    fn new() -> Self {
        Self {
            terms: MAX_TERMS,
            prefixes: MAX_PREFIX_TERMS,
        }
    }

    fn spend(&mut self, terms: &[Term]) -> Vec<Term> {
        terms
            .iter()
            .take(self.terms)
            .map(|term| {
                self.terms -= 1;
                let mut term = term.clone();
                match term.prefix && self.prefixes > 0 {
                    true => self.prefixes -= 1,
                    false => term.prefix = false,
                }
                term
            })
            .collect()
    }
}

impl Group {
    fn truncated(&self, budget: &mut Budget) -> Self {
        Self {
            positive: budget.spend(&self.positive),
            negative: budget.spend(&self.negative),
        }
    }

    fn render(&self) -> Option<String> {
        if self.positive.is_empty() {
            return None;
        }

        let positive = join(&self.positive, " AND ");
        if self.negative.is_empty() {
            return Some(positive);
        }

        Some(format!(
            "({positive}) NOT ({})",
            join(&self.negative, " OR ")
        ))
    }
}

fn join(terms: &[Term], separator: &str) -> String {
    terms
        .iter()
        .map(Term::render)
        .collect::<Vec<_>>()
        .join(separator)
}

fn groups(tokens: &[Token]) -> Vec<Group> {
    let mut groups = vec![Group::default()];

    for token in tokens {
        match token {
            Token::Or => groups.push(Group::default()),
            Token::Term(term) => {
                let group = groups.last_mut().expect("there is always a current group");
                if term.negated {
                    group.negative.push(term.clone());
                } else {
                    group.positive.push(term.clone());
                }
            }
        }
    }

    groups
}

#[derive(Debug)]
enum Token {
    Term(Term),
    Or,
}

fn tokenize(raw: &str) -> Vec<Token> {
    let chars: Vec<char> = raw.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    let mut negate_next = false;

    while at < chars.len() {
        if chars[at].is_whitespace() {
            at += 1;
            continue;
        }

        let mut negated = negate_next;
        negate_next = false;

        if chars[at] == '-' && chars.get(at + 1).is_some_and(|next| !next.is_whitespace()) {
            negated = true;
            at += 1;
        }

        let quoted = chars[at] == '"';
        let mut text = String::new();

        if quoted {
            at += 1;
            while at < chars.len() && chars[at] != '"' {
                text.push(chars[at]);
                at += 1;
            }
            at += usize::from(at < chars.len()); // the closing quote, when there was one
        } else {
            while at < chars.len() && !chars[at].is_whitespace() {
                text.push(chars[at]);
                at += 1;
            }
        }

        let mut prefix = false;
        if quoted {
            if chars.get(at) == Some(&'*') {
                prefix = true;
                at += 1;
            }
        } else if text.ends_with('*') {
            prefix = true;
            text = text.trim_end_matches('*').to_owned();
        }

        if !quoted && !negated {
            match text.as_str() {
                "OR" => {
                    tokens.push(Token::Or);
                    continue;
                }
                // The default, spelled out.
                "AND" => continue,
                "NOT" => {
                    negate_next = true;
                    continue;
                }
                _ => {}
            }
        }

        if text.chars().any(char::is_alphanumeric) {
            tokens.push(Token::Term(Term {
                text,
                quoted,
                prefix,
                negated,
            }));
        }
    }

    apply_trailing_prefix(&mut tokens);
    tokens
}

fn apply_trailing_prefix(tokens: &mut [Token]) {
    if let Some(Token::Term(term)) = tokens.last_mut()
        && !term.quoted
        && !term.negated
    {
        term.prefix = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ands_bare_words_and_prefixes_the_one_being_typed() {
        assert_eq!(
            fts_query("crab nebula"),
            Some(r#""crab" AND "nebula"*"#.into())
        );
        assert_eq!(fts_query("nebula"), Some(r#""nebula"*"#.into()));
    }

    #[test]
    fn a_quoted_phrase_stays_one_phrase() {
        assert_eq!(
            fts_query(r#""star cluster""#),
            Some(r#""star cluster""#.into())
        );
        assert_eq!(
            fts_query(r#"young "star cluster""#),
            Some(r#""young" AND "star cluster""#.into())
        );
    }

    #[test]
    fn a_quoted_phrase_is_never_prefixed_by_accident() {
        assert_eq!(
            fts_query(r#"m31 "deep field""#),
            Some(r#""m31" AND "deep field""#.into())
        );
    }

    #[test]
    fn an_unclosed_quote_still_produces_a_query() {
        assert_eq!(
            fts_query(r#""star cluster"#),
            Some(r#""star cluster""#.into())
        );
    }

    #[test]
    fn excludes_negated_terms() {
        assert_eq!(
            fts_query("galaxy -hubble"),
            Some(r#"("galaxy") NOT ("hubble")"#.into())
        );
        assert_eq!(
            fts_query(r#"galaxy -"deep field" -webb"#),
            Some(r#"("galaxy") NOT ("deep field" OR "webb")"#.into())
        );
        assert_eq!(
            fts_query("galaxy NOT hubble"),
            Some(r#"("galaxy") NOT ("hubble")"#.into())
        );
    }

    #[test]
    fn splits_groups_on_or() {
        assert_eq!(
            fts_query("comet OR asteroid"),
            Some(r#"("comet") OR ("asteroid"*)"#.into())
        );
        assert_eq!(
            fts_query("comet tail OR asteroid"),
            Some(r#"("comet" AND "tail") OR ("asteroid"*)"#.into())
        );
        assert_eq!(
            fts_query("comet -halley OR asteroid"),
            Some(r#"(("comet") NOT ("halley")) OR ("asteroid"*)"#.into())
        );
    }

    #[test]
    fn or_has_to_be_shouted_so_the_word_stays_searchable() {
        assert_eq!(
            fts_query("black or white"),
            Some(r#""black" AND "or" AND "white"*"#.into())
        );
        assert_eq!(fts_query("and"), Some(r#""and"*"#.into()));
    }

    #[test]
    fn honours_an_explicit_prefix_star() {
        assert_eq!(
            fts_query("neb* cloud"),
            Some(r#""neb"* AND "cloud"*"#.into())
        );
        assert_eq!(fts_query(r#""star clus"*"#), Some(r#""star clus"*"#.into()));
    }

    #[test]
    fn a_query_with_nothing_to_match_returns_nothing_rather_than_everything() {
        assert_eq!(fts_query("   "), None);
        assert_eq!(fts_query("!!! ???"), None);
        assert_eq!(
            fts_query("-hubble"),
            None,
            "an exclusion alone is not a search"
        );
        assert_eq!(fts_query("OR OR"), None);
    }

    #[test]
    fn a_nul_byte_cannot_terminate_the_match_expression() {
        let query = fts_query("\u{0}nebula").expect("the term survives, the NUL does not");
        assert!(!query.contains('\u{0}'), "{query:?}");
        assert_eq!(query, r#""nebula"*"#);

        for control in ["a\u{0}b", "a\u{1}b", "a\u{1f}b", "a\u{7f}b"] {
            let query = fts_query(control).unwrap_or_default();
            assert!(
                !query.chars().any(char::is_control),
                "{control:?} left a control character in {query:?}"
            );
        }
    }

    #[test]
    fn a_query_cannot_carry_more_terms_than_the_budget_allows() {
        let huge = "a* ".repeat(200);
        let query = fts_query(&huge).expect("something survives");
        assert_eq!(
            query.matches("\"a\"").count(),
            MAX_TERMS,
            "expected the tail to be dropped: {query}"
        );

        let spread = format!("{} OR {}", "a* ".repeat(50), "b* ".repeat(50));
        let query = fts_query(&spread).unwrap();
        let terms = query.matches("\"a\"").count() + query.matches("\"b\"").count();
        assert_eq!(terms, MAX_TERMS, "{query}");
    }

    #[test]
    fn only_a_few_terms_are_left_open_ended() {
        let query = fts_query(&"a* ".repeat(200)).unwrap();
        assert_eq!(
            query.matches(r#""a"*"#).count(),
            MAX_PREFIX_TERMS,
            "the rest must stay whole words: {query}"
        );
        assert_eq!(query.matches("\"a\"").count(), MAX_TERMS);
    }

    #[test]
    fn the_word_being_typed_is_still_a_prefix() {
        assert_eq!(fts_query("neb"), Some(r#""neb"*"#.into()));
        assert_eq!(
            fts_query("crab neb"),
            Some(r#""crab" AND "neb"*"#.into()),
            "the trailing word is what the prefix budget is for"
        );
    }

    #[test]
    fn an_ordinary_query_is_untouched_by_the_budget() {
        assert_eq!(
            fts_query("crab nebula"),
            Some(r#""crab" AND "nebula"*"#.into())
        );
    }

    #[test]
    fn user_input_never_becomes_fts_syntax() {
        for raw in [
            r#"crab" OR "x"#,
            "a*b",
            "(nebula)",
            "NEAR(a b)",
            r#"" OR entries_fts MATCH ""#,
            "^title:",
            "{a b}",
        ] {
            let query = fts_query(raw).unwrap_or_default();
            let outside_literals: String = query.split('"').step_by(2).collect();
            assert!(
                outside_literals
                    .chars()
                    .all(|c| c.is_whitespace() || "()*".contains(c) || c.is_ascii_uppercase()),
                "{raw:?} produced {query:?}, which has user text outside a string literal"
            );
        }
    }

    #[test]
    fn quotes_inside_a_term_are_dropped_rather_than_closing_it_early() {
        assert_eq!(
            fts_query(r#"crab" OR "x"#),
            Some(r#"("crab") OR ("x")"#.into())
        );
        assert_eq!(fts_query("a*b"), Some(r#""a*b"*"#.into()));
        assert_eq!(fts_query("(nebula)"), Some(r#""(nebula)"*"#.into()));
    }
}
