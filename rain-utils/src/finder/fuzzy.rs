const SCORE_MATCH: i32 = 16;
const SCORE_GAP_START: i32 = -3;
const SCORE_GAP_EXTENSION: i32 = -1;
const BONUS_BOUNDARY: i32 = 8;
const BONUS_BOUNDARY_WHITE: i32 = 10;
const BONUS_BOUNDARY_DELIMITER: i32 = 9;
const BONUS_CAMEL_123: i32 = 7;
const BONUS_CONSECUTIVE: i32 = 4;
const BONUS_FIRST_CHAR_MULTIPLIER: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuzzyMatch<'a> {
    pub text: &'a str,
    pub score: i32,
}

impl<'a> PartialOrd for FuzzyMatch<'a> {
    fn ge(&self, other: &Self) -> bool {
        self.score >= other.score
    }

    fn le(&self, other: &Self) -> bool {
        self.score <= other.score
    }

    fn lt(&self, other: &Self) -> bool {
        self.score < other.score
    }

    fn gt(&self, other: &Self) -> bool {
        self.score > other.score
    }

    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.score > other.score {
            Some(std::cmp::Ordering::Greater)
        } else if self.score == other.score {
            Some(std::cmp::Ordering::Equal)
        } else {
            Some(std::cmp::Ordering::Less)
        }
    }
}

impl<'a> Ord for FuzzyMatch<'a> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.score > other.score {
            std::cmp::Ordering::Greater
        } else if self.score == other.score {
            std::cmp::Ordering::Equal
        } else {
            std::cmp::Ordering::Less
        }
    }
}

impl<'a> FuzzyMatch<'a> {
    pub fn new(text: &'a str, score: i32) -> Self {
        Self { text, score }
    }
}

#[derive(Debug, Default)]
pub struct FuzzyMatcher {
    cost_table: Vec<i32>,
    consecutive_table: Vec<i32>,
    bonus_per_letter: Vec<i32>,
}

impl FuzzyMatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate the bounds to find the match more efficiently.
    pub fn max_and_min_viable_idx(
        &self,
        pattern_chars: &[char],
        text: &str,
    ) -> Option<(usize, usize)> {
        if text.is_empty() || pattern_chars.is_empty() {
            return None;
        }

        let mut min_idx: Option<usize> = None;
        let mut max_idx: Option<usize> = None;

        let &pattern_first_char = pattern_chars.first()?;
        let &pattern_last_char = pattern_chars.last()?;

        for (idx, letter) in text.chars().enumerate() {
            let mut letter_lower = letter.to_lowercase();
            if min_idx.is_none()
                && (letter == pattern_first_char || letter_lower.any(|c| c == pattern_first_char))
            {
                min_idx = Some(idx);
            }

            if letter == pattern_last_char || letter_lower.any(|c| c == pattern_last_char) {
                max_idx = Some(idx);
            }
        }

        let min_idx = min_idx?;
        let max_idx = max_idx?;

        if min_idx > max_idx {
            return None;
        }

        if (max_idx - min_idx) + 1 < pattern_chars.len() {
            return None;
        }

        Some((min_idx, max_idx))
    }

    fn valid_text_and_calculate_context_bonus(
        &mut self,
        pattern_chars: &[char],
        text_chars: &[char],
    ) -> bool {
        self.bonus_per_letter.clear();
        self.bonus_per_letter.reserve(text_chars.len());

        // first match always get this bonus
        self.bonus_per_letter.push(BONUS_BOUNDARY_WHITE);

        let mut pattern_idx = 0;
        for pair in text_chars.windows(2) {
            let prev_char = pair[0];
            let text_char = pair[1];

            let bonus = match prev_char {
                ' ' => BONUS_BOUNDARY_WHITE,
                '-' | '/' | ':' | ';' | ',' => BONUS_BOUNDARY_DELIMITER,
                _ => {
                    if (prev_char.is_lowercase() && text_char.is_uppercase())
                        || (prev_char.is_alphabetic() && text_char.is_numeric())
                    {
                        BONUS_CAMEL_123
                    } else if !prev_char.is_ascii_alphanumeric()
                        && text_char.is_ascii_alphanumeric()
                    {
                        BONUS_BOUNDARY
                    } else {
                        0
                    }
                }
            };

            self.bonus_per_letter.push(bonus);

            if let Some(&pattern_char) = pattern_chars.get(pattern_idx) {
                if text_char.to_lowercase().any(|c| c == pattern_char) {
                    pattern_idx += 1;
                }
            }
        }

        // if all pattern chars were consumed, then is valid
        pattern_idx == pattern_chars.len()
    }

    pub fn match_score(&mut self, pattern_chars: &[char], text: &str) -> i32 {
        let Some((min_idx, max_idx)) = self.max_and_min_viable_idx(&pattern_chars, text) else {
            return 0;
        };

        let chars_vec: Vec<char> = text.chars().collect();
        let text_chars = &chars_vec[min_idx..=max_idx];

        if !self.valid_text_and_calculate_context_bonus(pattern_chars, &chars_vec) {
            return 0;
        }

        // add more row and column to keep a initial state 0
        let width = text_chars.len() + 1;
        let height = pattern_chars.len() + 1;

        self.cost_table.clear();
        self.cost_table.resize_with(width * height, || 0);

        self.consecutive_table.clear();
        self.consecutive_table.resize_with(width * height, || 0);

        for pattern_idx in 0..pattern_chars.len() {
            let pattern_char = pattern_chars[pattern_idx];
            let row = pattern_idx + 1;

            let max_text_idx_viable = text_chars.len() - (pattern_chars.len() - pattern_idx);

            for text_idx in pattern_idx..=max_text_idx_viable {
                let text_char = text_chars[text_idx];
                let column = text_idx + 1;

                let mut bonus = 0;
                let is_match = text_char.to_lowercase().any(|c| c == pattern_char);
                let mut consecutive = self.consecutive_table[(row - 1) * width + (column - 1)];

                if is_match {
                    consecutive += 1;

                    let letter_bonus = self.bonus_per_letter[min_idx + text_idx];
                    let mut consecutive_bonus = 0;
                    let mut head_bonus = 0;

                    if consecutive > 1 {
                        consecutive_bonus = BONUS_CONSECUTIVE;
                        head_bonus =
                            self.bonus_per_letter[min_idx + text_idx - (consecutive as usize - 1)];
                    }

                    bonus = letter_bonus.max(consecutive_bonus).max(head_bonus);

                    if pattern_idx == 0 {
                        let first_text_char = chars_vec[0];
                        if pattern_char == first_text_char {
                            bonus *= BONUS_FIRST_CHAR_MULTIPLIER;
                        }
                    }
                    bonus += SCORE_MATCH;
                } else {
                    consecutive = 0;
                }

                let diag_score = self
                    .cost_table
                    .get((row - 1) * width + column - 1)
                    .copied()
                    .unwrap_or(0);

                let s1 = if is_match { diag_score + bonus } else { 0 };

                let left_score = self.cost_table[row * width + column - 1];
                let left_is_a_match = self.consecutive_table[row * width + column - 1] > 0;

                let s2 = if left_is_a_match {
                    left_score + SCORE_GAP_START
                } else {
                    left_score + SCORE_GAP_EXTENSION
                };

                let best_cell = s1.max(s2).max(0);

                self.cost_table[row * width + column] = best_cell;
                self.consecutive_table[row * width + column] =
                    if is_match && best_cell == s1 && best_cell > 0 {
                        consecutive
                    } else {
                        0
                    };
            }
        }

        let last_row = (height - 1) * width;
        let match_score = self.cost_table[last_row..]
            .iter()
            .max()
            .copied()
            .unwrap_or(0);

        match_score
    }

    pub fn rank<'a>(&mut self, pattern_chars: &[char], texts: &'a [&str]) -> Vec<FuzzyMatch<'a>> {
        let mut rank = Vec::with_capacity(texts.len());

        for text in texts {
            let score = self.match_score(pattern_chars, text);
            rank.push(FuzzyMatch { text, score });
        }

        rank
    }

    pub fn order_by_rank<'a>(
        &mut self,
        pattern_chars: &[char],
        texts: &'a [&str],
    ) -> Vec<FuzzyMatch<'a>> {
        let mut scored: Vec<FuzzyMatch> = texts
            .iter()
            .map(|&text| FuzzyMatch {
                text,
                score: self.match_score(pattern_chars, text),
            })
            .collect();

        scored.sort_unstable();

        scored
    }
}
