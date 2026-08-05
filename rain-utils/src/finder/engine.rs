use super::fuzzy::{FuzzyMatch, FuzzyMatcher};

#[derive(Debug, Default)]
pub struct FuzzyEngine<'a> {
    matcher: FuzzyMatcher,
    last_pattern: String,
    cached_candidates: Vec<&'a str>,
    next_cached_candidates: Vec<&'a str>,
    all_candidates: &'a [&'a str],
}

impl<'a> FuzzyEngine<'a> {
    pub fn new(candidates: &'a [&'a str]) -> Self {
        Self {
            matcher: FuzzyMatcher::new(),
            last_pattern: String::from(""),

            // last calculated candidates
            cached_candidates: Vec::new(),

            // to avoid reallocations, will use double buffer
            next_cached_candidates: Vec::new(),
            all_candidates: candidates,
        }
    }

    pub fn search(&mut self, pattern: &str, sort_by_score: bool) -> Vec<FuzzyMatch<'a>> {
        let mut matches = Vec::new();
        self.next_cached_candidates.clear();

        let candidates = if !self.last_pattern.is_empty() && pattern.starts_with(&self.last_pattern)
        {
            // reduces the search area with cache
            &self.cached_candidates
        } else {
            self.all_candidates
        };

        let pattern_chars: Vec<char> = pattern.chars().flat_map(|c| c.to_lowercase()).collect();

        for &candidate in candidates {
            let score = self.matcher.match_score(&pattern_chars, candidate);
            if score > 0 {
                matches.push(FuzzyMatch::new(candidate, score));
                self.next_cached_candidates.push(candidate);
            }
        }

        if sort_by_score {
            matches.sort_unstable_by(|a, b| b.cmp(&a));
        }

        std::mem::swap(
            &mut self.cached_candidates,
            &mut self.next_cached_candidates,
        );

        self.last_pattern = pattern.to_string();

        matches
    }
}
