use super::matcher::Matcher;
use super::sanitize::remove_accent;
use super::types::Match;

/// An engine for executing fuzzy searches.
///
/// This engine reuses internal buffers to avoid extra allocations and
/// implements an FZF-like algorithm to find the best matches without losing
/// flexibility or performance.
///
/// Moreover, the [`FuzzyEngine`] can keep a cache to reduce the search space
/// and improve the performance with the pattern with same init.
#[derive(Debug, Default)]
pub struct Engine<'a> {
    matcher: Matcher,
    last_pattern: String,
    pattern_buffer: Vec<char>,
    pattern_bytes_buffer: Vec<u8>,
    cached_candidates: Vec<(&'a str, usize)>,
    next_cached_candidates: Vec<(&'a str, usize)>,
    all_candidates: &'a [(&'a str, usize)],
}

impl<'a> Engine<'a> {
    pub fn new(candidates: &'a [(&'a str, usize)]) -> Self {
        Self {
            matcher: Matcher::new(),
            last_pattern: String::from(""),
            pattern_buffer: Vec::new(),
            pattern_bytes_buffer: Vec::new(),

            // last calculated candidates
            cached_candidates: Vec::new(),

            // to avoid reallocations, will use double buffer
            next_cached_candidates: Vec::new(),
            all_candidates: candidates,
        }
    }

    pub fn search(&mut self, pattern: &str, sort_by_score: bool) -> Vec<Match> {
        let mut matches = Vec::new();

        // avoid cache invalidation
        if pattern.is_empty() {
            return matches;
        }

        self.next_cached_candidates.clear();

        let candidates = if !self.last_pattern.is_empty() && pattern.starts_with(&self.last_pattern)
        {
            // reduces the search area with cache
            &self.cached_candidates
        } else {
            self.cached_candidates.clear();
            self.all_candidates
        };

        self.pattern_buffer.clear();
        self.pattern_bytes_buffer.clear();

        let mut is_ascii = true;
        for c in pattern.chars() {
            is_ascii = is_ascii && c.is_ascii();

            // sanitize accents and uppercase to lowercase
            let lower_c = remove_accent(c);
            self.pattern_buffer.push(lower_c);
            if is_ascii {
                self.pattern_bytes_buffer.push(lower_c as u8);
            }
        }

        for (candidate, candidate_id) in candidates {
            let text_is_ascii = is_ascii && candidate.is_ascii();

            let score = self.matcher.match_score(
                candidate,
                &self.pattern_buffer,
                &self.pattern_bytes_buffer,
                text_is_ascii,
            );
            if score > 0 {
                matches.push(Match::new(*candidate_id, score));
                self.next_cached_candidates.push((candidate, *candidate_id));
            }
        }

        if sort_by_score {
            matches.sort_unstable_by(|a, b| b.cmp(&a));
        }

        std::mem::swap(
            &mut self.cached_candidates,
            &mut self.next_cached_candidates,
        );

        self.last_pattern.clear();
        self.last_pattern.push_str(pattern);

        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_cache_invalidation_and_correctness() {
        let dataset = vec![
            ("rain-utils/src/finder/fuzzy.rs", 1),
            ("rain-launcher/src/main.rs", 2),
            ("target/", 3),
            ("LICENSE", 4),
        ];

        let mut engine = Engine::new(&dataset);

        // first char is "r" (first user key)
        let res_1 = engine.search("r", false);
        assert_eq!(res_1.len(), 3, "Must find the 3 way with 'r'.");
        assert_eq!(
            engine.cached_candidates.len(),
            3,
            "The cache must keep the 3 candidates."
        );

        // user press "ra" (will active the cache)
        let res_2 = engine.search("ra", false);
        assert_eq!(res_2.len(), 2, "Must have 2 ways with 'ra'.");

        // user press "rain" (Keep with 2)
        let res_3 = engine.search("rain", false);
        assert_eq!(res_3.len(), 2, "The 2 files starts with 'rain'.");

        // user press "rainf" (just fuzzy.rs must stay)
        let res_4 = engine.search("rainf", false);
        assert_eq!(res_4.len(), 1, "Just 'fuzzy.rs' has the letters 'rainf'.");
        assert_eq!(res_4[0].id, 1);

        // clean the cache
        let res_5 = engine.search("license", false);
        assert_eq!(
            res_5.len(),
            1,
            "Must resets the cache and find the 'LICENSE'."
        );
        assert_eq!(res_5[0].id, 4);
    }

    #[test]
    fn test_engine_empty_pattern_behavior() {
        let dataset = vec![("src/main.rs", 1)];
        let mut engine = Engine::new(&dataset);

        // a empty pattern not should crash
        let res = engine.search("", false);
        assert_eq!(res.len(), 0, "A empty pattern must returns 0 results.");
    }
}
