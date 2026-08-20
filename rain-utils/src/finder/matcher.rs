use super::sanitize::remove_accent;
use super::types::{Match, ScoreCell};

const SCORE_MATCH: i32 = 16;
const SCORE_GAP_START: i32 = -3;
const SCORE_GAP_EXTENSION: i32 = -1;
const BONUS_BOUNDARY: i32 = 8;
const BONUS_BOUNDARY_WHITE: i32 = 10;
const BONUS_BOUNDARY_DELIMITER: i32 = 9;
const BONUS_CAMEL_123: i32 = 7;
const BONUS_CONSECUTIVE: i32 = 4;
const BONUS_FIRST_CHAR_MULTIPLIER: i32 = 2;

/// A backend to perform searches and calculate text similarity scores.
///
/// This struct implements an FZF-like algorithm. It reuses internal buffers
/// across multiple search queries to avoid memory reallocations.
///
/// Use [`Matcher::match_score`] to execute the algorithm and obtain an [`i32`]
/// representing the text similarity score.
#[derive(Debug, Default)]
pub struct Matcher {
    prev_row: Vec<ScoreCell>,
    current_row: Vec<ScoreCell>,
    bonus_per_letter: Vec<i32>,
    char_buffer: Vec<char>,
}

impl Matcher {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculates the context bonus for each character in the bounded text.
    ///
    /// The FZF algorithm uses the preceding character to calculate a positional
    /// bonus for the current character. This method populates the `char_buffer`
    /// with the sliced text and calculates the corresponding scores into
    /// `bonus_per_letter`.
    fn calculate_score_context(
        &mut self,
        text: &str,
        init_byte: usize,
        finish_byte: usize,
    ) {
        self.char_buffer.clear();
        let mut first_char = ' ';

        if init_byte == 0 {
            self.char_buffer
                .extend(text[..finish_byte].chars());
        } else {
            first_char = text[..init_byte]
                .chars()
                .next_back()
                .unwrap_or(' ');

            self.char_buffer
                .extend(text[init_byte..finish_byte].chars());
        }

        self.bonus_per_letter.clear();
        self.bonus_per_letter
            .reserve((finish_byte - init_byte) + 1);

        if let Some(&first_interval_letter) = self.char_buffer.first() {
            self.bonus_per_letter
                .push(get_context_bonus(first_char, first_interval_letter));
        }

        for pair in self.char_buffer.windows(2) {
            self.bonus_per_letter
                .push(get_context_bonus(pair[0], pair[1]));
        }

        // sanitize the buffer
        for c in self.char_buffer.iter_mut() {
            *c = remove_accent(*c);
        }
    }

    /// Calculates the similarity score of `text` against `pattern_chars`.
    ///
    /// # Performance
    ///
    /// For optimal performance, set the `is_ascii` flag to `true` if both the
    /// `pattern` and the `text` are purely ASCII. Set it to `false` for UTF-8
    /// strings.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rain_utils::finder::matcher::Matcher;
    ///
    /// let text = "Hello, world!";
    /// let pattern = vec!['h', 'w'];
    /// let pattern_bytes: Vec<u8> = pattern
    ///     .iter()
    ///     .map(|c| c.to_ascii_lowercase() as u8)
    ///     .collect();
    ///
    /// let mut matcher = Matcher::new();
    /// let score = matcher.match_score(text, &pattern, &pattern_bytes, true);
    /// ```
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

        let text_chars = &self.char_buffer;

        // add more row and column to keep a initial state 0
        let width = text_chars.len() + 1;

        self.prev_row.resize(width, ScoreCell::default());
        self.current_row
            .resize(width, ScoreCell::default());

        self.prev_row.fill(ScoreCell::default());

        for pattern_idx in 0..pattern_chars.len() {
            let pattern_char = pattern_chars[pattern_idx];

            let max_text_idx_viable =
                text_chars.len() - (pattern_chars.len() - pattern_idx);

            // avoid old data on left neighbor (column - 1)
            self.current_row[pattern_idx] = ScoreCell::default();

            for text_idx in pattern_idx..=max_text_idx_viable {
                let text_char = text_chars[text_idx];
                let column = text_idx + 1;

                let mut bonus = 0;
                let is_match = text_char == pattern_char;
                let mut consecutive = self.prev_row[column - 1].consecutives;

                if is_match {
                    consecutive += 1;

                    let letter_bonus = self.bonus_per_letter[text_idx];
                    let mut consecutive_bonus = 0;
                    let mut head_bonus = 0;

                    if consecutive > 1 {
                        consecutive_bonus = BONUS_CONSECUTIVE;
                        head_bonus = self.bonus_per_letter
                            [text_idx - (consecutive as usize - 1)];
                    }

                    bonus = letter_bonus
                        .max(consecutive_bonus)
                        .max(head_bonus);

                    if pattern_idx == 0 {
                        if pattern_char == self.char_buffer[0] {
                            bonus *= BONUS_FIRST_CHAR_MULTIPLIER;
                        }
                    }
                    bonus += SCORE_MATCH;
                } else {
                    consecutive = 0;
                }

                let diag_score = self.prev_row[column - 1].score;

                let s1 = if is_match { diag_score + bonus } else { 0 };

                let left_score = self.current_row[column - 1].score;

                let left_is_a_match =
                    self.current_row[column - 1].consecutives > 0;

                let s2 = if left_is_a_match {
                    left_score + SCORE_GAP_START
                } else {
                    left_score + SCORE_GAP_EXTENSION
                };

                let best_cell = s1.max(s2).max(0);

                self.current_row[column].score = best_cell;

                self.current_row[column].consecutives =
                    if is_match && best_cell == s1 && best_cell > 0 {
                        consecutive
                    } else {
                        0
                    };
            }

            std::mem::swap(&mut self.current_row, &mut self.prev_row);
        }

        // let last_row = (height - 1) * width;
        let fist_valid_col = pattern_chars.len();

        let match_score = self.prev_row[fist_valid_col..]
            .iter()
            .max_by_key(|v| v.score)
            .map(|v| v.score)
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
            let score =
                self.match_score(text, pattern_chars, pattern_bytes, is_ascii);
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
                    self.match_score(
                        text,
                        pattern_chars,
                        pattern_bytes,
                        is_ascii,
                    ),
                )
            })
            .collect();

        scored.sort_unstable();

        scored
    }
}

/// Calculates the bounds to find the match more efficiently.
///
/// The bounds are represents by a tuple with two [`usize`] when the first
/// is the min byte index (inclusive) and the second is the last byte index
/// (exclusive).
///
/// If the text are impossible to match like: The pattern does not fit; the
/// pattern chars are not in `text`, and so on... Then the bounds are
/// impossibles and returns a `None`.
///
/// # Examples
///
/// To use the indices with the `&str` just use:
///
/// ```rust,ignore
/// let text = "Hello, world!";
/// let pattern_chars = vec!['h', 'w'];
/// let pattern_bytes: Vec<u8> = pattern_chars
///     .iter()
///     .map(|c| c.to_ascii_lowercase() as u8)
///     .collect();
///
/// let (min, max) = find_viable_bounds(
///     text,
///     &pattern_chars,
///     &pattern_bytes,
///     true
/// ).unwrap();
///
/// let text_viable = &text[min..max];
/// ```
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

fn find_viable_bounds_ascii(
    text_bytes: &[u8],
    pattern_bytes: &[u8],
) -> Option<(usize, usize)> {
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

    // an ascii char has a 1 byte size
    let max_idx_exclusive = max_idx + 1;

    Some((min_idx, max_idx_exclusive))
}

fn find_viable_bounds_utf8(
    text: &str,
    pattern_chars: &[char],
) -> Option<(usize, usize)> {
    let &pattern_first_char = pattern_chars.first()?;
    let &pattern_last_char = pattern_chars.last()?;

    let mut min_idx_opt: Option<usize> = None;
    let mut max_idx_opt: Option<usize> = None;
    let mut max_char_len = 0;

    let pattern_len = pattern_chars.len();

    let mut pattern_idx: usize = 0;

    // find the min_idx
    for (byte_idx, letter) in text.char_indices() {
        let letter_clean = remove_accent(letter);

        if letter_clean == pattern_first_char {
            min_idx_opt = Some(byte_idx);
            break;
        }
    }

    let min_byte_idx = min_idx_opt?;

    // find the max_idx
    for (byte_idx, letter) in text.char_indices().rev() {
        if byte_idx < min_byte_idx {
            return None;
        }

        let letter_clean = remove_accent(letter);

        if letter_clean == pattern_last_char {
            max_idx_opt = Some(byte_idx);
            max_char_len = letter.len_utf8(); // save the size in bytes
            break;
        }
    }

    let max_byte_idx = max_idx_opt?;

    // the max_idx can not be greater than min_idx
    if max_byte_idx < min_byte_idx {
        return None;
    }

    let max_byte_exclusive = max_byte_idx + max_char_len;
    let viable_slice = &text[min_byte_idx..max_byte_exclusive];

    // verify if has all pattern chars
    for letter in viable_slice.chars() {
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

    Some((min_byte_idx, max_byte_exclusive))
}

fn get_context_bonus(prev: char, current: char) -> i32 {
    match prev {
        ' ' => BONUS_BOUNDARY_WHITE,
        '-' | '/' | ':' | ';' | ',' => BONUS_BOUNDARY_DELIMITER,
        _ => {
            if (prev.is_lowercase() && current.is_uppercase())
                || (prev.is_alphabetic() && current.is_numeric())
            {
                BONUS_CAMEL_123
            } else if !prev.is_ascii_alphanumeric()
                && current.is_ascii_alphanumeric()
            {
                BONUS_BOUNDARY
            } else {
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_ascii_and_pattern_bytes(
        pattern: &[char],
        text: &str,
    ) -> (bool, Vec<u8>) {
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
        let (is_ascii, pattern_bytes) =
            is_ascii_and_pattern_bytes(&pattern, &text);

        let score =
            matcher.match_score(text, &pattern, &pattern_bytes, is_ascii);
        assert!(score > 0, "Must find the ASCII substring.");
    }

    #[test]
    fn test_unicode_case_insensitivity() {
        let mut matcher = Matcher::new();
        let text = "ui/Match_Ação.rs";
        let pattern: Vec<char> = "açao".chars().map(remove_accent).collect();
        let (is_ascii, pattern_bytes) =
            is_ascii_and_pattern_bytes(&pattern, &text);

        let score =
            matcher.match_score(text, &pattern, &pattern_bytes, is_ascii);
        assert!(score > 0, "Must decode UTF-8 and find 'Ação' with 'açao'.");
    }

    #[test]
    fn test_early_rejection() {
        let mut matcher = Matcher::new();
        let text = "usr/lib/waybar/config.json";
        let pattern: Vec<char> = "zsh".chars().collect();
        let (is_ascii, pattern_bytes) =
            is_ascii_and_pattern_bytes(&pattern, &text);

        let score =
            matcher.match_score(text, &pattern, &pattern_bytes, is_ascii);
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
        let (is_ascii_1, pattern_bytes_1) =
            is_ascii_and_pattern_bytes(&pattern, text_1);
        let score_good =
            matcher.match_score(text_1, &pattern, &pattern_bytes_1, is_ascii_1);

        // in middle of string
        let text_2 = "src/profile.rs";
        let (is_ascii_2, pattern_bytes_2) =
            is_ascii_and_pattern_bytes(&pattern, text_2);
        let score_bad =
            matcher.match_score(text_2, &pattern, &pattern_bytes_2, is_ascii_2);

        assert!(
            score_good > score_bad,
            "FuzzyMatcher must prioritize characters after Boundary limiters."
        );
    }

    #[test]
    fn test_buffer_reuse_ghost_data_leak() {
        let mut matcher = Matcher::new();

        let pattern: Vec<char> = "core".chars().collect();
        let (is_ascii, pattern_bytes) =
            is_ascii_and_pattern_bytes(&pattern, "core");

        // calculate the baseline score for a short string with a fresh matcher.
        let text_short = "src/core.rs";
        let score_baseline =
            matcher.match_score(text_short, &pattern, &pattern_bytes, is_ascii);

        // pollute the matcher's internal buffers with a very long string.
        // This forces the `cost_table` or `current_row` to expand its capacity
        // and leaves high scores in memory indices far beyond the short string's length.
        let text_long = "src/modules/core_engine/utils/core_manager_core.rs";
        let _score_long =
            matcher.match_score(text_long, &pattern, &pattern_bytes, is_ascii);

        // Re-evaluate the short string using the now polluted matcher.
        // If `current_row` is not properly cleared, or if the left boundary check
        // (text_idx > pattern_idx) fails, the algorithm will read ghost scores
        // from the previous long evaluation, artificially inflating the final score.
        let score_polluted =
            matcher.match_score(text_short, &pattern, &pattern_bytes, is_ascii);

        // The score must be perfectly identical to the baseline.
        assert_eq!(
            score_baseline, score_polluted,
            "Ghost data leak detected! The score changed after reusing the buffer on a longer string."
        );
    }
}
