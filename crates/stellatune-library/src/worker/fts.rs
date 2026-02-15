pub(super) fn build_fts_query(q: &str) -> String {
    // Simple prefix query:
    //   "foo bar" => "foo* AND bar*"
    q.split_whitespace()
        .filter(|s| !s.is_empty())
        // Always quote tokens so that punctuation (e.g. apostrophes) won't break the FTS5 parser.
        .filter_map(|raw| {
            let token =
                raw.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string();
            if token.is_empty() {
                return None;
            }

            // Escape double-quotes inside the token per SQLite rules.
            // See: https://www.sqlite.org/lang_expr.html (string literal escaping) and FTS5 query syntax.
            let token = token.replace('"', "\"\"");
            Some(format!("\"{token}\"*"))
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

#[cfg(test)]
mod tests {
    use super::build_fts_query;

    #[test]
    fn build_fts_query_quotes_tokens() {
        assert_eq!(build_fts_query("chu'meng"), "\"chu'meng\"*");
        assert_eq!(build_fts_query("hello world"), "\"hello\"* AND \"world\"*");
        assert_eq!(build_fts_query(r#"D:\CloudMusic"#), r#""D:\CloudMusic"*"#);
    }
}
