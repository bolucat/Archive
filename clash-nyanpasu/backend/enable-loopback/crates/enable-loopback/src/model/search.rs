//! Cached, locale-independent search over raw values, never rendered placeholders.

// Product relevance weights, in display/copy column order. Names matter most; paths
// and SIDs remain searchable without overwhelming human-readable application names.
pub const DISPLAY_NAME_WEIGHT: u32 = 10;
pub const DESCRIPTION_WEIGHT: u32 = 3;
pub const PACKAGE_WEIGHT: u32 = 5;
pub const AC_NAME_WEIGHT: u32 = 6;
pub const SID_WEIGHT: u32 = 1;
pub const USERS_WEIGHT: u32 = 2;
pub const BINARIES_WEIGHT: u32 = 1;
const WEIGHTS: [u32; 7] = [
    DISPLAY_NAME_WEIGHT,
    DESCRIPTION_WEIGHT,
    PACKAGE_WEIGHT,
    AC_NAME_WEIGHT,
    SID_WEIGHT,
    USERS_WEIGHT,
    BINARIES_WEIGHT,
];

#[derive(Clone, Debug)]
pub struct SearchFields([Option<String>; 7]);

impl SearchFields {
    /// Call once when loading/replacing a row, before any query is entered.
    pub fn new(fields: [Option<String>; 7]) -> Self {
        Self(fields.map(|field| field.map(|value| value.to_lowercase())))
    }
}

#[derive(Debug)]
pub struct Query {
    tokens: Vec<String>,
}

impl Query {
    pub fn new(text: &str) -> Self {
        Self {
            tokens: text.split_whitespace().map(str::to_lowercase).collect(),
        }
    }

    /// Scores use half-points: quality 3/2/1.5/1 is represented as 6/4/3/2.
    /// None means at least one token was absent from every column (AND semantics).
    pub fn score(&self, fields: &SearchFields) -> Option<u32> {
        self.tokens.iter().try_fold(0, |sum, token| {
            let best = fields
                .0
                .iter()
                .zip(WEIGHTS)
                .filter_map(|(field, weight)| {
                    field.as_deref().map(|field| weight * quality(field, token))
                })
                .max()
                .unwrap_or(0);
            (best > 0).then_some(sum + best)
        })
    }
}

fn quality(field: &str, token: &str) -> u32 {
    if field == token {
        return 6;
    }
    if field.starts_with(token) {
        return 4;
    }
    let mut found = false;
    // Inspect every occurrence: a later word-start outranks an earlier substring.
    for (index, _) in field.match_indices(token) {
        found = true;
        if field[..index]
            .chars()
            .next_back()
            .is_some_and(|ch| matches!(ch, ' ' | '.' | '_' | '-' | '\\' | '/' | '{'))
        {
            return 3;
        }
    }
    if found { 2 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(index: usize, value: &str) -> SearchFields {
        let mut values = std::array::from_fn(|_| None);
        values[index] = Some(value.into());
        SearchFields::new(values)
    }

    #[test]
    fn all_columns_have_documented_weights_and_quality_levels() {
        for (column, weight) in WEIGHTS.into_iter().enumerate() {
            let query = Query::new("store");
            for (text, quality) in [("Store", 6), ("Store app", 4), ("xstore", 2)] {
                assert_eq!(query.score(&fields(column, text)), Some(weight * quality));
            }
            for separator in [' ', '.', '_', '-', '\\', '/', '{'] {
                let text = format!("xstore{separator}store");
                assert_eq!(query.score(&fields(column, &text)), Some(weight * 3));
            }
            assert_eq!(query.score(&fields(column, "x:store")), Some(weight * 2));
        }
    }

    #[test]
    fn tokens_are_and_across_columns_and_take_max_not_sum() {
        let fields = SearchFields::new([
            Some("Store".into()),
            None,
            Some("Store package".into()),
            None,
            None,
            Some("DOMAIN\\Alice".into()),
            None,
        ]);
        assert_eq!(Query::new(" STORE\t Alice\n").score(&fields), Some(60 + 6));
        assert_eq!(Query::new("store missing").score(&fields), None);
        assert_eq!(Query::new("\u{2003} \n").score(&fields), Some(0));
    }

    #[test]
    fn unicode_lowercase_null_empty_and_placeholders() {
        assert_eq!(Query::new("ÄПП").score(&fields(0, "äпп")), Some(60));
        assert_eq!(Query::new("İ").score(&fields(0, "İ")), Some(60));
        let missing = SearchFields::new(std::array::from_fn(|_| None));
        for text in ["(No DisplayName)", "(None)", "description", "sid"] {
            assert_eq!(Query::new(text).score(&missing), None);
        }
        assert_eq!(Query::new("app").score(&fields(0, "")), None);
        assert_eq!(Query::new("").score(&missing), Some(0));
    }
}
