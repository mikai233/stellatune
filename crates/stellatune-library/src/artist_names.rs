use std::collections::HashSet;

use anyhow::Result;

/// Text tag separators. Do not treat '&', 'and' or 'feat.' as delimiters:
/// they can be part of an artist's credited name.
pub(crate) fn normalize_artists_json(values: &str, fallback: Option<&str>) -> Result<String> {
    let values: Vec<String> = serde_json::from_str(values)?;
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut append = |value: &str| {
        for part in value.split([
            '/', '／', '、', ',', '，', ';', '；', '|', '｜', '\0', '\r', '\n',
        ]) {
            let name = part.trim();
            if !name.is_empty() && seen.insert(name.to_owned()) {
                names.push(name.to_owned());
            }
        }
    };
    if values.iter().any(|v| !v.trim().is_empty()) {
        for value in &values {
            append(value);
        }
    } else if let Some(value) = fallback {
        append(value);
    }
    Ok(serde_json::to_string(&names)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_common_separators_and_deduplicates_in_credit_order() {
        assert_eq!(
            normalize_artists_json("[]", Some(" A/B ／ C、D,E，F;G；H|I｜J\nK\rL\0M / A ;; "))
                .unwrap(),
            r#"["A","B","C","D","E","F","G","H","I","J","K","L","M"]"#
        );
        assert_eq!(
            normalize_artists_json(
                r#"[" 初音ミク / 鏡音リン ","初音ミク","Guest"]"#,
                Some("Ignored fallback")
            )
            .unwrap(),
            r#"["初音ミク","鏡音リン","Guest"]"#
        );
    }

    #[test]
    fn retains_names_without_explicit_separators_and_empty_metadata() {
        assert_eq!(normalize_artists_json("[]", None).unwrap(), "[]");
        assert_eq!(
            normalize_artists_json("[]", Some("Simon & Garfunkel")).unwrap(),
            r#"["Simon & Garfunkel"]"#
        );
        assert_eq!(
            normalize_artists_json("[]", Some("A feat. B")).unwrap(),
            r#"["A feat. B"]"#
        );
    }
}
