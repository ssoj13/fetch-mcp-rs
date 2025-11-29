use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Search match with context
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchMatch {
    /// Matched text
    pub matched_text: String,

    /// Line number (1-indexed)
    pub line_number: usize,

    /// Character position in line
    pub position: usize,

    /// Context before match
    pub context_before: String,

    /// Context after match
    pub context_after: String,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResult {
    /// Search query
    pub query: String,

    /// Total matches found
    pub total_matches: usize,

    /// Individual matches
    pub matches: Vec<SearchMatch>,

    /// Whether search was case-sensitive
    pub case_sensitive: bool,

    /// Whether regex was used
    pub is_regex: bool,

    /// Total occurrences count (for simple pattern matching)
    pub total_occurrences: Option<usize>,

    /// Unique words extracted from content (optional)
    pub unique_words: Option<Vec<String>>,
}

/// Search options
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Case-sensitive search
    pub case_sensitive: bool,

    /// Use regex pattern
    pub use_regex: bool,

    /// Maximum number of matches to return (0 = unlimited)
    pub max_matches: usize,

    /// Number of characters for context before/after match
    pub context_chars: usize,

    /// Search only in specific lines (line numbers, 1-indexed)
    pub line_filter: Option<Vec<usize>>,

    /// Extract unique words from content
    pub extract_words: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            max_matches: 100,
            context_chars: 50,
            line_filter: None,
            extract_words: false,
        }
    }
}

/// Search for text in content
pub fn search_in_text(content: &str, query: &str, options: SearchOptions) -> Result<SearchResult> {
    if query.is_empty() {
        anyhow::bail!("Search query cannot be empty");
    }

    let mut matches = Vec::new();

    if options.use_regex {
        // Regex search
        let pattern = if options.case_sensitive {
            query.to_string()
        } else {
            format!("(?i){}", query)
        };

        let re = Regex::new(&pattern).context("Invalid regex pattern")?;
        matches = search_with_regex(content, &re, options.context_chars, options.line_filter.as_deref());
    } else {
        // Plain text search
        matches = search_plain_text(
            content,
            query,
            options.case_sensitive,
            options.context_chars,
            options.line_filter.as_deref(),
        );
    }

    // Apply max_matches limit
    let total_matches = matches.len();
    if options.max_matches > 0 && matches.len() > options.max_matches {
        matches.truncate(options.max_matches);
    }

    // Count total occurrences for non-regex searches
    let total_occurrences = if !options.use_regex {
        Some(count_occurrences(content, query, options.case_sensitive))
    } else {
        None
    };

    // Extract unique words if requested
    let unique_words = if options.extract_words {
        Some(extract_unique_words(content))
    } else {
        None
    };

    Ok(SearchResult {
        query: query.to_string(),
        total_matches,
        matches,
        case_sensitive: options.case_sensitive,
        is_regex: options.use_regex,
        total_occurrences,
        unique_words,
    })
}

/// Search using plain text
fn search_plain_text(
    content: &str,
    query: &str,
    case_sensitive: bool,
    context_chars: usize,
    line_filter: Option<&[usize]>,
) -> Vec<SearchMatch> {
    let mut matches = Vec::new();

    let _search_content = if case_sensitive {
        content.to_string()
    } else {
        content.to_lowercase()
    };

    let search_query = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    for (line_idx, line) in content.lines().enumerate() {
        let line_number = line_idx + 1;

        // Apply line filter if specified
        if let Some(filter) = line_filter {
            if !filter.contains(&line_number) {
                continue;
            }
        }

        let search_line = if case_sensitive {
            line.to_string()
        } else {
            line.to_lowercase()
        };

        let mut start_pos = 0;
        while let Some(pos) = search_line[start_pos..].find(&search_query) {
            let abs_pos = start_pos + pos;
            let matched_text = line[abs_pos..abs_pos + query.len()].to_string();

            let (context_before, context_after) = extract_context(line, abs_pos, query.len(), context_chars);

            matches.push(SearchMatch {
                matched_text,
                line_number,
                position: abs_pos,
                context_before,
                context_after,
            });

            start_pos = abs_pos + query.len();
        }
    }

    matches
}

/// Search using regex
fn search_with_regex(
    content: &str,
    re: &Regex,
    context_chars: usize,
    line_filter: Option<&[usize]>,
) -> Vec<SearchMatch> {
    let mut matches = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        let line_number = line_idx + 1;

        // Apply line filter if specified
        if let Some(filter) = line_filter {
            if !filter.contains(&line_number) {
                continue;
            }
        }

        for mat in re.find_iter(line) {
            let matched_text = mat.as_str().to_string();
            let position = mat.start();

            let (context_before, context_after) = extract_context(line, position, matched_text.len(), context_chars);

            matches.push(SearchMatch {
                matched_text,
                line_number,
                position,
                context_before,
                context_after,
            });
        }
    }

    matches
}

/// Extract context around a match
fn extract_context(line: &str, match_pos: usize, match_len: usize, context_chars: usize) -> (String, String) {
    let before_start = match_pos.saturating_sub(context_chars);
    let before = line[before_start..match_pos].to_string();

    let after_start = match_pos + match_len;
    let after_end = (after_start + context_chars).min(line.len());
    let after = line[after_start..after_end].to_string();

    (before, after)
}

/// Count total occurrences of a query in content
pub fn count_occurrences(content: &str, query: &str, case_sensitive: bool) -> usize {
    let search_content = if case_sensitive {
        content
    } else {
        &content.to_lowercase()
    };

    let search_query = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    search_content.matches(&search_query).count()
}

/// Extract all unique words from content
pub fn extract_unique_words(content: &str) -> Vec<String> {
    use std::collections::HashSet;

    let word_re = Regex::new(r"\b\w+\b").unwrap();
    let words: HashSet<String> = word_re
        .find_iter(content)
        .map(|m| m.as_str().to_lowercase())
        .collect();

    let mut word_list: Vec<String> = words.into_iter().collect();
    word_list.sort();
    word_list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_plain_text() {
        let content = "Hello world\nHello Rust\nGoodbye world";

        let options = SearchOptions {
            case_sensitive: false,
            use_regex: false,
            ..Default::default()
        };

        let result = search_in_text(content, "hello", options).unwrap();
        assert_eq!(result.total_matches, 2);
        assert_eq!(result.matches[0].line_number, 1);
        assert_eq!(result.matches[1].line_number, 2);
    }

    #[test]
    fn test_search_case_sensitive() {
        let content = "Hello world\nhello rust";

        let options = SearchOptions {
            case_sensitive: true,
            use_regex: false,
            ..Default::default()
        };

        let result = search_in_text(content, "Hello", options).unwrap();
        assert_eq!(result.total_matches, 1);
        assert_eq!(result.matches[0].line_number, 1);
    }

    #[test]
    fn test_search_regex() {
        let content = "test123\ntest456\nabc789";

        let options = SearchOptions {
            case_sensitive: false,
            use_regex: true,
            ..Default::default()
        };

        let result = search_in_text(content, r"test\d+", options).unwrap();
        assert_eq!(result.total_matches, 2);
    }

    #[test]
    fn test_search_with_context() {
        let content = "The quick brown fox jumps over the lazy dog";

        let options = SearchOptions {
            context_chars: 10,
            ..Default::default()
        };

        let result = search_in_text(content, "fox", options).unwrap();
        assert_eq!(result.matches[0].matched_text, "fox");
        assert!(result.matches[0].context_before.contains("brown"));
        assert!(result.matches[0].context_after.contains("jumps"));
    }

    #[test]
    fn test_search_max_matches() {
        let content = "a\na\na\na\na";

        let options = SearchOptions {
            max_matches: 2,
            ..Default::default()
        };

        let result = search_in_text(content, "a", options).unwrap();
        assert_eq!(result.total_matches, 5);
        assert_eq!(result.matches.len(), 2);
    }

    #[test]
    fn test_count_occurrences() {
        let content = "hello world hello rust hello";
        assert_eq!(count_occurrences(content, "hello", false), 3);
        assert_eq!(count_occurrences(content, "HELLO", false), 3);
        assert_eq!(count_occurrences(content, "HELLO", true), 0);
    }

    #[test]
    fn test_extract_unique_words() {
        let content = "hello world hello rust world";
        let words = extract_unique_words(content);
        assert_eq!(words, vec!["hello", "rust", "world"]);
    }
}
