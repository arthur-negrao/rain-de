use super::sanitize::remove_accent;
use super::types::Match;

const SCORE_MATCH: i32 = 16;
const SCORE_GAP_START: i32 = -3;
const SCORE_GAP_EXTENSION: i32 = -1;
const BONUS_BOUNDARY: i32 = 8;
const BONUS_BOUNDARY_WHITE: i32 = 10;
const BONUS_BOUNDARY_DELIMITER: i32 = 9;
const BONUS_CAMEL_123: i32 = 7;
const BONUS_CONSECUTIVE: i32 = 4;
const BONUS_FIRST_CHAR_MULTIPLIER: i32 = 2;

#[derive(Debug, Default)]
pub struct Matcher {
    cost_table: Vec<i32>,
    consecutive_table: Vec<i32>,
    bonus_per_letter: Vec<i32>,
    char_buffer: Vec<char>,
}

impl Matcher {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self::default()
    }

    fn calculate_score_context(&mut self, text: &str, init: usize, finish: usize) {
        self.char_buffer.clear();
        let mut first_char = ' ';

        if init == 0 {
            self.char_buffer.extend(text.chars().take(finish + 1));
        } else {
            let mut iter = text.chars().skip(init - 1);

            if let Some(c) = iter.next() {
                first_char = c;
            }

            self.char_buffer.extend(iter.take((finish - init) + 1));
        }

        self.bonus_per_letter.clear();
        self.bonus_per_letter.reserve((finish - init) + 1);

        // first match always get this bonus
        //
        let get_bonus = |prev: char, current: char| -> i32 {
            match prev {
                ' ' => BONUS_BOUNDARY_WHITE,
                '-' | '/' | ':' | ';' | ',' => BONUS_BOUNDARY_DELIMITER,
                _ => {
                    if (prev.is_lowercase() && current.is_uppercase())
                        || (prev.is_alphabetic() && current.is_numeric())
                    {
                        BONUS_CAMEL_123
                    } else if !prev.is_ascii_alphanumeric() && current.is_ascii_alphanumeric() {
                        BONUS_BOUNDARY
                    } else {
                        0
                    }
                }
            }
        };

        if let Some(&first_interval_letter) = self.char_buffer.first() {
            self.bonus_per_letter
                .push(get_bonus(first_char, first_interval_letter));
        }

        for pair in self.char_buffer.windows(2) {
            self.bonus_per_letter.push(get_bonus(pair[0], pair[1]));
        }
    }

    fn sanitize_char_buffer(&mut self) {
        for c in self.char_buffer.iter_mut() {
            *c = remove_accent(*c);
        }
    }

    /// Calculates the `text` score by the `pattern_chars` similarity.
    pub fn match_score(
        &mut self,
        text: &str,
        pattern_chars: &[char],
        pattern_bytes: &[u8],
        is_ascii: bool,
    ) -> i32 {
        let Some((min_idx, max_idx)) =
            find_viable_bounds(text, &pattern_chars, pattern_bytes, is_ascii)
        else {
            return 0;
        };

        self.calculate_score_context(text, min_idx, max_idx);

        self.sanitize_char_buffer();

        let text_chars = &self.char_buffer;

        // add more row and column to keep a initial state 0
        let width = text_chars.len() + 1;
        let height = pattern_chars.len() + 1;

        self.cost_table.resize(width * height, 0);
        self.consecutive_table.resize(width * height, 0);

        for pattern_idx in 0..pattern_chars.len() {
            let pattern_char = pattern_chars[pattern_idx];
            let row = pattern_idx + 1;

            let max_text_idx_viable = text_chars.len() - (pattern_chars.len() - pattern_idx);

            for text_idx in pattern_idx..=max_text_idx_viable {
                let text_char = text_chars[text_idx];
                let column = text_idx + 1;

                let mut bonus = 0;
                let is_match = text_char == pattern_char;
                let mut consecutive = self.consecutive_table[(row - 1) * width + (column - 1)];

                if is_match {
                    consecutive += 1;

                    let letter_bonus = self.bonus_per_letter[text_idx];
                    let mut consecutive_bonus = 0;
                    let mut head_bonus = 0;

                    if consecutive > 1 {
                        consecutive_bonus = BONUS_CONSECUTIVE;
                        head_bonus = self.bonus_per_letter[text_idx - (consecutive as usize - 1)];
                    }

                    bonus = letter_bonus.max(consecutive_bonus).max(head_bonus);

                    if pattern_idx == 0 {
                        if pattern_char == self.char_buffer[0] {
                            bonus *= BONUS_FIRST_CHAR_MULTIPLIER;
                        }
                    }
                    bonus += SCORE_MATCH;
                } else {
                    consecutive = 0;
                }

                let diag_score = self.cost_table[(row - 1) * width + column - 1];

                let s1 = if is_match { diag_score + bonus } else { 0 };

                // the row (pattern_idx - 1) can not be greater than the column
                // (text_idx - 1)
                let left_is_valid = pattern_idx <= (column - 1);
                let left_score = if left_is_valid {
                    self.cost_table[row * width + column - 1]
                } else {
                    0
                };

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
        let fist_valid_col = pattern_chars.len();

        let match_score = self.cost_table[last_row + fist_valid_col..]
            .iter()
            .max()
            .copied()
            .unwrap_or(0);

        match_score
    }

    /// Ranks the `texts` by the `pattern_chars`.
    pub fn rank(
        &mut self,
        texts: &[(&str, usize)],
        pattern_chars: &[char],
        pattern_bytes: &[u8],
        is_ascii: bool,
    ) -> Vec<Match> {
        let mut rank = Vec::with_capacity(texts.len());

        for (text, id) in texts {
            let score = self.match_score(text, pattern_chars, pattern_bytes, is_ascii);
            rank.push(Match::new(*id, score));
        }

        rank
    }

    /// Ranks the `texts` by the `pattern_chars` and sort them in descendent
    /// order by the score.
    ///
    /// # Cautions
    ///
    /// This method uses a unstable sorter.
    pub fn rank_and_sort(
        &mut self,
        texts: &[(&str, usize)],
        pattern_chars: &[char],
        pattern_bytes: &[u8],
        is_ascii: bool,
    ) -> Vec<Match> {
        let mut scored: Vec<Match> = texts
            .iter()
            .map(|(text, id)| {
                Match::new(
                    *id,
                    self.match_score(text, pattern_chars, pattern_bytes, is_ascii),
                )
            })
            .collect();

        scored.sort_unstable();

        scored
    }
}

/// Calculates the bounds to find the match more efficiently.
#[inline]
fn find_viable_bounds(
    text: &str,
    pattern_chars: &[char],
    pattern_bytes: &[u8],
    is_ascii: bool,
) -> Option<(usize, usize)> {
    if text.is_empty() || pattern_chars.is_empty() {
        return None;
    }

    if is_ascii {
        let text_bytes = text.as_bytes();
        return find_viable_bounds_ascii(text_bytes, pattern_bytes);
    }

    find_viable_bounds_utf8(text, pattern_chars)
}

fn find_viable_bounds_ascii(text_bytes: &[u8], pattern_bytes: &[u8]) -> Option<(usize, usize)> {
    let &pattern_first_byte = pattern_bytes.first()?;
    let &pattern_last_byte = pattern_bytes.last()?;

    let pattern_len = pattern_bytes.len();
    let text_len = text_bytes.len();

    let mut min_idx_opt: Option<usize> = None;
    let mut max_idx_opt: Option<usize> = None;

    let mut pattern_idx: usize = 0;

    // find the min_idx
    for (idx, &letter) in text_bytes.iter().enumerate() {
        let letter_lower = letter.to_ascii_lowercase();

        if letter_lower == pattern_first_byte {
            min_idx_opt = Some(idx);
            break;
        }
    }

    let min_idx = min_idx_opt?;

    // find the max_idx
    for (idx, &letter) in text_bytes[min_idx..].iter().rev().enumerate() {
        let real_idx = text_len - idx - 1;

        // the max_idx can not be greater than min_idx
        if real_idx < min_idx {
            return None;
        }

        let letter_lower = letter.to_ascii_lowercase();

        if letter_lower == pattern_last_byte {
            max_idx_opt = Some(real_idx);
            break;
        }
    }

    let max_idx = max_idx_opt?;

    if max_idx < min_idx {
        return None;
    }

    for &letter in &text_bytes[min_idx..=max_idx] {
        let letter_lower = letter.to_ascii_lowercase();

        if pattern_idx < pattern_len {
            if pattern_bytes[pattern_idx] == letter_lower {
                pattern_idx += 1;
            }
        } else {
            break;
        }
    }

    if pattern_idx != pattern_len {
        return None;
    }

    Some((min_idx, max_idx))
}

fn find_viable_bounds_utf8(text: &str, pattern_chars: &[char]) -> Option<(usize, usize)> {
    let &pattern_first_char = pattern_chars.first()?;
    let &pattern_last_char = pattern_chars.last()?;

    let mut min_idx_opt: Option<usize> = None;
    let mut max_idx_opt: Option<usize> = None;

    let pattern_len = pattern_chars.len();
    let text_len = text.chars().count();

    let mut pattern_idx: usize = 0;

    // find the min_idx
    for (idx, letter) in text.chars().enumerate() {
        let letter_clean = remove_accent(letter);

        if letter_clean == pattern_first_char {
            min_idx_opt = Some(idx);
            break;
        }
    }

    let min_idx = min_idx_opt?;

    // find the max_idx
    for (idx, letter) in text.chars().rev().enumerate() {
        let real_idx = text_len - idx - 1;
        if real_idx < min_idx {
            return None;
        }

        let letter_clean = remove_accent(letter);

        if letter_clean == pattern_last_char {
            max_idx_opt = Some(real_idx);
            break;
        }
    }

    let max_idx = max_idx_opt?;

    // the max_idx can not be greater than min_idx
    if max_idx < min_idx {
        return None;
    }

    // verify if has all pattern chars
    for (idx, letter) in text.chars().enumerate() {
        if idx < min_idx {
            continue;
        }

        if idx > max_idx {
            break;
        }

        let letter_clean = remove_accent(letter);

        if pattern_idx < pattern_len {
            if pattern_chars[pattern_idx] == letter_clean {
                pattern_idx += 1;
            }
        } else {
            break;
        }
    }

    if pattern_idx != pattern_len {
        return None;
    }

    Some((min_idx, max_idx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_ascii_and_pattern_bytes(pattern: &[char], text: &str) -> (bool, Vec<u8>) {
        let is_ascii = text.is_ascii() && pattern.iter().all(|c| c.is_ascii());
        let pattern_bytes: Vec<u8> = pattern
            .iter()
            .map(|c| c.to_ascii_lowercase() as u8)
            .collect();

        (is_ascii, pattern_bytes)
    }

    #[test]
    fn test_exact_match_ascii() {
        let mut matcher = Matcher::new();
        let text = "src/app.rs";
        let pattern: Vec<char> = "app".chars().collect();
        let (is_ascii, pattern_bytes) = is_ascii_and_pattern_bytes(&pattern, &text);

        let score = matcher.match_score(text, &pattern, &pattern_bytes, is_ascii);
        assert!(score > 0, "Must find the ASCII substring.");
    }

    #[test]
    fn test_unicode_case_insensitivity() {
        let mut matcher = Matcher::new();
        let text = "ui/Match_Ação.rs";
        let pattern: Vec<char> = "açao".chars().map(remove_accent).collect();
        let (is_ascii, pattern_bytes) = is_ascii_and_pattern_bytes(&pattern, &text);

        let score = matcher.match_score(text, &pattern, &pattern_bytes, is_ascii);
        assert!(score > 0, "Must decode UTF-8 and find 'Ação' with 'açao'.");
    }

    #[test]
    fn test_early_rejection() {
        let mut matcher = Matcher::new();
        let text = "usr/lib/waybar/config.json";
        let pattern: Vec<char> = "zsh".chars().collect();
        let (is_ascii, pattern_bytes) = is_ascii_and_pattern_bytes(&pattern, &text);

        let score = matcher.match_score(text, &pattern, &pattern_bytes, is_ascii);
        assert_eq!(
            score, 0,
            "Must return 0 instantly if the substring does not exist."
        );
    }

    #[test]
    fn test_bonus_scoring_logic() {
        let mut matcher = Matcher::new();
        let pattern: Vec<char> = "file".chars().collect();

        // post the slash (limiter)
        let text_1 = "src/file_manager.rs";
        let (is_ascii_1, pattern_bytes_1) = is_ascii_and_pattern_bytes(&pattern, text_1);
        let score_good = matcher.match_score(text_1, &pattern, &pattern_bytes_1, is_ascii_1);

        // in middle of string
        let text_2 = "src/profile.rs";
        let (is_ascii_2, pattern_bytes_2) = is_ascii_and_pattern_bytes(&pattern, text_2);
        let score_bad = matcher.match_score(text_2, &pattern, &pattern_bytes_2, is_ascii_2);

        assert!(
            score_good > score_bad,
            "FuzzyMatcher must prioritize characters after Boundary limiters."
        );
    }
}
