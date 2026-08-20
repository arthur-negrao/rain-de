use std::iter::FusedIterator;
use std::ops::{BitAnd, BitOr, BitXor, Not};

/// A dynamically sized, dense bitset.
///
/// This structure tracks the presence of components using bits packed into
/// 64-bit blocks. It provides zero-allocation lazy iterators for complex set
/// operations, making it highly efficient for frame-to-frame engine queries.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BitSet {
    bit_blocks: Vec<u64>,
    active_count: usize,
}

impl BitSet {
    /// Creates a new `BitSet` from an existing slice of 64-bit blocks.
    ///
    /// This method calculates the initial active bit count by iterating over
    /// the provided slice. It is useful for deserialization or cloning raw
    /// states.
    ///
    /// # Complexity
    ///
    /// *O(n)* where *n* is the length of the slice.
    pub fn new(bits: &[u64]) -> Self {
        let bits_vec = Vec::from(bits);

        Self {
            bit_blocks: bits_vec,
            active_count: bits
                .iter()
                .map(|block| block.count_ones() as usize)
                .sum(),
        }
    }

    /// Creates an empty `BitSet` with pre-allocated capacity for at least `max_bits`.
    ///
    /// The underlying storage will not reallocate until it needs to accommodate
    /// an index strictly greater than the requested capacity. Since bits are
    /// packed into 64-bit blocks, the actual allocated capacity may be rounded
    /// up to a multiple of 64.
    ///
    /// # Complexity
    ///
    /// *O(n)* to initialize the underlying memory with zeros.
    pub fn with_capacity(max_bits: usize) -> Self {
        let blocks_needed = (max_bits + 63) / 64;

        Self {
            bit_blocks: vec![0; blocks_needed],
            active_count: 0,
        }
    }

    /// Reserves capacity for at least `additional_bits` more bits to be inserted.
    ///
    /// The allocation is done in 64-bit blocks, meaning the new capacity
    /// may be larger than strictly requested. Newly allocated bits are initialized to `0`.
    ///
    /// # Complexity
    /// *O(n)*.
    pub fn reserve(&mut self, additional_bits: usize) {
        let new_bits = (additional_bits + 63) / 64;

        self.bit_blocks.reserve(new_bits);
    }

    /// Sets the bit at the specified index to `1`.
    ///
    /// If the set did not previously contain this value, the internal count is
    /// incremented. The underlying storage will automatically resize to
    /// accommodate the given index.
    ///
    /// # Complexity
    /// Average *O(1)*.
    /// Worst (Reallocation) *O(n)*.
    ///
    /// # Examples
    /// ```
    /// # use rain_utils::collections::BitSet;
    ///
    /// let mut set = BitSet::default();
    /// set.set(42);
    /// assert!(set.test(42));
    /// ```
    pub fn set(&mut self, index: usize) {
        let block = Self::get_block_index(index);
        let bit_index = Self::get_index_on_block(index);

        self.ensure_block(index, block);

        let bitmask = 1u64 << bit_index;

        let was_unset = (self.bit_blocks[block] & bitmask) == 0;
        self.active_count += was_unset as usize;

        // set the bit to 1
        self.bit_blocks[block] |= bitmask;
    }

    /// Sets the bit at the specified index to `1` without guards.
    ///
    /// If the set did not previously contain this value, the internal count is
    /// incremented. The underlying storage will not automatically resize to
    /// accommodate the given index (unsafe).
    ///
    /// # Safety
    ///
    /// Calling this method with an out-of-bounds index results in *undefined
    /// behavior* (UB).
    ///
    /// # Complexity
    ///
    /// Average *O(1)*.
    pub unsafe fn set_unchecked(&mut self, index: usize) {
        let block = Self::get_block_index(index);
        let bit_index = Self::get_index_on_block(index);

        let bitmask = 1u64 << bit_index;

        let block_ref = unsafe { self.bit_blocks.get_unchecked_mut(block) };

        let was_unset = (*block_ref & bitmask) == 0;
        self.active_count += was_unset as usize;

        // set the bit to 1
        *block_ref |= bitmask;
    }

    /// Sets all allocated bits in the set to `1`.
    ///
    /// # Complexity
    ///
    /// *O(n)*
    pub fn set_all(&mut self) {
        let bit_mask = !0u64;

        for block in self.bit_blocks.iter_mut() {
            *block = bit_mask;
        }

        self.active_count = self.bit_capacity();
    }

    /// Resets a bit to `0`.
    ///
    /// If the index is out of bounds, this function does nothing.
    ///
    /// # Complexity
    ///
    /// *O(1)*.
    pub fn reset(&mut self, index: usize) {
        // unallocated block are 0 by default
        if index >= self.bit_capacity() {
            return;
        }

        let block = Self::get_block_index(index);
        let index_on_block = Self::get_index_on_block(index);

        let bitmask = !(1u64 << index_on_block);

        let was_set = (self.bit_blocks[block] & bitmask) != 0;
        self.active_count -= was_set as usize;

        // set the bit to 0
        self.bit_blocks[block] &= bitmask;
    }

    /// Resets a bit to `0`.
    ///
    /// # Safety
    ///
    /// Calling this method with an out-of-bounds index results in *undefined
    /// behavior* (UB).
    ///
    /// # Complexity
    ///
    /// *O(1)*.
    pub unsafe fn reset_unchecked(&mut self, index: usize) {
        let block = Self::get_block_index(index);
        let index_on_block = Self::get_index_on_block(index);

        let bitmask = !(1u64 << index_on_block);

        let block_ref = unsafe { self.bit_blocks.get_unchecked_mut(block) };

        let was_set = (*block_ref & bitmask) != 0;
        self.active_count -= was_set as usize;

        // set the bit to 0
        *block_ref &= bitmask;
    }

    /// Resets all allocated bits to `0`.
    ///
    /// # Complexity
    /// *O(n)*
    pub fn reset_all(&mut self) {
        for block in self.bit_blocks.iter_mut() {
            *block = 0;
        }

        self.active_count = 0;
    }

    /// Flips a bit from `0` to `1` or from `1` to `0`.
    ///
    /// If the set did not previously contain this value, the internal count is
    /// incremented. The underlying storage will automatically resize to
    /// accommodate the given index.
    ///
    /// # Complexity
    /// Average *O(1)*.
    /// Worst (Reallocation) *O(n)*.
    pub fn flip(&mut self, index: usize) {
        let block = Self::get_block_index(index);
        let index_on_block = Self::get_index_on_block(index);

        self.ensure_block(index, block);

        let bitmask = 1u64 << index_on_block;
        let was_unset = (self.bit_blocks[block] & bitmask) == 0;

        if was_unset {
            self.active_count += 1;
        } else {
            self.active_count -= 1;
        }

        // flip the bit with a XOR gate
        self.bit_blocks[block] ^= bitmask;
    }

    /// Flips all allocated bits.
    ///
    /// # Complexity
    /// *O(n)*.
    pub fn flip_all(&mut self) {
        for block in self.bit_blocks.iter_mut() {
            *block = !(*block);
        }

        self.active_count = self.bit_capacity() - self.count();
    }

    /// Returns `true` if the set contains the specified value.
    ///
    /// # Complexity
    /// *O(1)*.
    pub fn test(&self, index: usize) -> bool {
        if index >= self.bit_capacity() {
            return false;
        }

        let block = Self::get_block_index(index);
        let bitmask = Self::get_bitmask(index);

        (self.bit_blocks[block] & bitmask) != 0
    }

    /// Returns `true` if the set contains the specified value.
    ///
    /// # Safety
    ///
    /// Calling this method with an out-of-bounds index results in *undefined
    /// behavior* (UB).
    ///
    /// # Complexity
    ///
    /// *O(1)*.
    #[inline]
    pub unsafe fn test_unchecked(&self, index: usize) -> bool {
        let block = Self::get_block_index(index);
        let bitmask = Self::get_bitmask(index);

        let block_ref = unsafe { self.bit_blocks.get_unchecked(block) };

        (*block_ref & bitmask) != 0
    }

    /// Returns `true` if no bits are set to `0` (i.e., the set is completely
    /// empty).
    ///
    /// # Complexity
    /// *O(1)*
    pub fn none(&self) -> bool {
        self.count() == 0
    }

    /// Returns `true` if at least one bit is set to `1`.
    ///
    /// # Complexity
    /// *O(1)*.
    pub fn any(&self) -> bool {
        self.count() != 0
    }

    /// Returns `true` if all allocated bits are set to `1`.
    ///
    /// # Complexity
    /// *O(1)*.
    pub fn all(&self) -> bool {
        self.count() == self.bit_capacity()
    }

    /// Ensures the index can be storage in the memory.
    ///
    /// This method ensure a reallocation if needed and set new bits with `0`.
    ///
    /// # Complexity
    ///
    /// *O(n)* if reallocation needed.
    fn ensure_block(&mut self, index: usize, block: usize) {
        if index >= self.bit_capacity() {
            let mut new_len = self.bit_blocks.len().max(1);

            while new_len <= block {
                new_len *= 2;
            }

            self.bit_blocks.resize(new_len, 0); // set to 0 all new bits
        }
    }

    /// Returns an iterator over the indices of all set bits.
    ///
    /// # Complexity
    /// *O(n)*.
    #[inline]
    pub fn iter(&self) -> BitSetIter<'_> {
        self.into_iter()
    }

    /// Returns a lazy iterator yielding the indices of bits set in both this
    /// set and `other`.
    ///
    /// This represents the bitwise `AND` (intersection) operation.
    ///
    /// # Complexity
    /// All iterations *O(n)*.
    /// Each iteration *O(1)*.
    ///
    /// # Memory
    /// *O(1)*.
    pub fn iter_and<'a>(&'a self, other: &'a BitSet) -> BitAndIter<'a> {
        BitAndIter {
            left_blocks: &self.bit_blocks,
            right_blocks: &other.bit_blocks,
            current_block: 0,
            current_block_idx: 0,
        }
    }

    /// Returns a lazy iterator yielding the indices of bits set in both this
    /// set and `other`.
    ///
    /// This represents the bitwise `OR` (union) operation.
    ///
    /// # Complexity
    /// All iterations *O(n)*.
    /// Each iteration *O(1)*.
    ///
    /// # Memory
    /// *O(1)*.
    pub fn iter_or<'a>(&'a self, other: &'a BitSet) -> BitOrIter<'a> {
        BitOrIter {
            left_blocks: &self.bit_blocks,
            right_blocks: &other.bit_blocks,
            current_block: 0,
            current_block_idx: 0,
        }
    }

    /// Returns a lazy iterator yielding the indices of bits set in both this
    /// set and `other`.
    ///
    /// This represents the bitwise `XOR` (symmetric difference) operation.
    ///
    /// # Complexity
    /// All iterations *O(n)*.
    /// Each iteration *O(1)*.
    ///
    /// # Memory
    /// *O(1)*.
    pub fn iter_xor<'a>(&'a self, other: &'a BitSet) -> BitXorIter<'a> {
        BitXorIter {
            left_blocks: &self.bit_blocks,
            right_blocks: &other.bit_blocks,
            current_block: 0,
            current_block_idx: 0,
        }
    }

    /// Returns a lazy iterator yielding the indices of bits set in both this
    /// set and `other`.
    ///
    /// This represents the bitwise `AND NOT` (difference) operation.
    ///
    /// # Complexity
    /// All iterations *O(n)*.
    /// Each iteration *O(1)*.
    ///
    /// # Memory
    /// *O(1)*.
    pub fn iter_without<'a>(&'a self, other: &'a BitSet) -> BitAndNotIter<'a> {
        BitAndNotIter {
            left_blocks: &self.bit_blocks,
            right_blocks: &other.bit_blocks,
            current_block: 0,
            current_block_idx: 0,
        }
    }

    /// Returns number of bits set to `1`.
    ///
    /// # Complexity
    /// *O(1)*.
    #[inline]
    pub fn count(&self) -> usize {
        self.active_count
    }

    /// Returns the logical capacity of the set in bits.
    ///
    /// This value represents the current upper bound of manageable indices
    /// before a reallocation is required. Since bits are stored in 64-bit blocks,
    /// this number is always a multiple of 64.
    ///
    /// # Complexity
    /// *O(1)*.
    #[inline]
    pub fn bit_capacity(&self) -> usize {
        self.bit_blocks.len() * 64
    }

    /// Returns the max theoretical capacity of the internal vector in bits.
    ///
    /// This represents the raw allocated capacity. Because bits are packed
    /// into 64-bit blocks, this value is always a multiple of 64.
    ///
    /// # Complexity
    /// *O(1)*.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.bit_blocks.capacity() * 64
    }

    /// Creates a BitSet directly from a raw bitmask.
    ///
    /// # Examples:
    /// ```
    /// # use rain_utils::collections::BitSet;
    ///
    /// let bitset = BitSet::from_bitmask(0b101); // sets indices 0 and 2.
    /// ```
    ///
    /// # Complexity
    /// *O(1)*.
    pub fn from_bitmask(value: u64) -> Self {
        let active_count = value.count_ones() as usize;
        let bit_blocks = vec![value];

        Self {
            bit_blocks,
            active_count,
        }
    }

    #[inline]
    const fn get_index_on_block(index: usize) -> usize {
        index % 64
    }

    #[inline]
    const fn get_bitmask(index: usize) -> u64 {
        1 << Self::get_index_on_block(index)
    }

    #[inline]
    const fn get_block_index(index: usize) -> usize {
        index / 64
    }
}

impl From<&[u8]> for BitSet {
    /// Creates a `BitSet` from a bytes slice.
    ///
    /// Each bit in slice is converts to a bit in BitSet. If the number of
    /// elements in slice is not multiple of 64, the new bits will set to 0 to
    /// fit in a 64-bit block.
    ///
    /// # Complexity
    /// *O(n)*.
    fn from(bytes: &[u8]) -> Self {
        let mut active_count: usize = 0;
        let mut bit_blocks = Vec::with_capacity((bytes.len() + 7) / 8);

        for chunk in bytes.chunks(8) {
            let mut buffer = [0u8; 8];

            buffer[..chunk.len()].copy_from_slice(chunk);
            let buffer_block = u64::from_le_bytes(buffer);

            active_count += buffer_block.count_ones() as usize;

            bit_blocks.push(buffer_block);
        }

        Self {
            bit_blocks: bit_blocks,
            active_count: active_count,
        }
    }
}

impl Not for BitSet {
    type Output = Self;

    fn not(mut self) -> Self::Output {
        self.flip_all();
        self
    }
}

impl Not for &BitSet {
    type Output = BitSet;

    fn not(self) -> Self::Output {
        let mut cloned = self.clone();
        cloned.flip_all();

        cloned
    }
}

impl BitAnd for BitSet {
    type Output = Self;
    fn bitand(mut self, rhs: Self) -> Self::Output {
        self.bit_blocks.truncate(rhs.bit_blocks.len());

        let mut active_count: usize = 0;

        for (left_block, right_block) in
            self.bit_blocks.iter_mut().zip(&rhs.bit_blocks)
        {
            *left_block &= right_block;
            active_count += left_block.count_ones() as usize;
        }

        self.active_count = active_count;

        self
    }
}

impl BitAnd for &BitSet {
    type Output = BitSet;
    fn bitand(self, rhs: Self) -> Self::Output {
        let mut cloned = self.clone();
        cloned.bit_blocks.truncate(rhs.bit_blocks.len());

        let mut active_count: usize = 0;

        for (left_block, right_block) in
            cloned.bit_blocks.iter_mut().zip(&rhs.bit_blocks)
        {
            *left_block &= right_block;
            active_count += left_block.count_ones() as usize;
        }

        cloned.active_count = active_count;

        cloned
    }
}

impl BitOr for BitSet {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        let (min, mut max) = if self.bit_blocks.len() < rhs.bit_blocks.len() {
            (self, rhs)
        } else {
            (rhs, self)
        };

        for (left_block, right_block) in
            min.bit_blocks.iter().zip(&mut max.bit_blocks)
        {
            *right_block |= left_block;
        }

        // O(n)
        max.active_count = max
            .bit_blocks
            .iter()
            .map(|block| block.count_ones() as usize)
            .sum();

        max
    }
}

impl BitOr for &BitSet {
    type Output = BitSet;
    fn bitor(self, rhs: Self) -> Self::Output {
        let (min, mut max) = if self.bit_blocks.len() < rhs.bit_blocks.len() {
            (self, rhs.clone())
        } else {
            (rhs, self.clone())
        };

        for (left_block, right_block) in
            min.bit_blocks.iter().zip(&mut max.bit_blocks)
        {
            *right_block |= left_block;
        }

        // O(n)
        max.active_count = max
            .bit_blocks
            .iter()
            .map(|block| block.count_ones() as usize)
            .sum();

        max
    }
}

impl BitXor for BitSet {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self::Output {
        let (min, mut max) = if self.bit_blocks.len() < rhs.bit_blocks.len() {
            (self, rhs)
        } else {
            (rhs, self)
        };

        for (left_block, right_block) in
            min.bit_blocks.iter().zip(&mut max.bit_blocks)
        {
            *right_block ^= left_block;
        }

        // O(n)
        max.active_count = max
            .bit_blocks
            .iter()
            .map(|block| block.count_ones() as usize)
            .sum();

        max
    }
}

impl BitXor for &BitSet {
    type Output = BitSet;
    fn bitxor(self, rhs: Self) -> Self::Output {
        let (min, mut max) = if self.bit_blocks.len() < rhs.bit_blocks.len() {
            (self, rhs.clone())
        } else {
            (rhs, self.clone())
        };

        for (left_block, right_block) in
            min.bit_blocks.iter().zip(&mut max.bit_blocks)
        {
            *right_block ^= left_block;
        }

        // O(n)
        max.active_count = max
            .bit_blocks
            .iter()
            .map(|block| block.count_ones() as usize)
            .sum();

        max
    }
}

impl<'a> IntoIterator for &'a BitSet {
    type Item = usize;
    type IntoIter = BitSetIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        BitSetIter {
            bitset: self,
            current_block: self.bit_blocks.first().copied().unwrap_or(0u64),
            current_block_idx: 0,
        }
    }
}

/// An iterator over the set bits of a [`BitSet`].
///
/// This struct is created by the `iter` method on [`BitSet`].
/// It yields the global indices of all bits set to `1` in ascending order.
#[derive(Clone)]
pub struct BitSetIter<'a> {
    bitset: &'a BitSet,
    current_block: u64,
    current_block_idx: usize,
}

impl<'a> Iterator for BitSetIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        // ignore the bits 0
        while self.current_block == 0 {
            self.current_block_idx += 1;

            if self.current_block_idx >= self.bitset.bit_blocks.len() {
                return None;
            }

            self.current_block = self.bitset.bit_blocks[self.current_block_idx];
        }

        let bit_local_pos = self.current_block.trailing_zeros() as usize;
        let bit_global_pos = (64 * self.current_block_idx) + bit_local_pos;

        // reset the analyzed bit
        self.current_block ^= 1u64 << bit_local_pos;

        Some(bit_global_pos)
    }
}

/// A lazy iterator yielding the intersection (Bitwise AND) of two bitsets.
///
/// This struct is created by the `iter_and` method on [`BitSet`].
#[derive(Clone)]
pub struct BitAndIter<'a> {
    left_blocks: &'a [u64],
    right_blocks: &'a [u64],
    current_block: u64,
    current_block_idx: usize,
}

/// A lazy iterator yielding the union (Bitwise OR) of two bitsets.
///
/// This struct is created by the `iter_or` method on [`BitSet`].
#[derive(Clone)]
pub struct BitOrIter<'a> {
    left_blocks: &'a [u64],
    right_blocks: &'a [u64],
    current_block: u64,
    current_block_idx: usize,
}

/// A lazy iterator yielding the symmetric difference (Bitwise XOR) of two bitsets.
///
/// This struct is created by the `iter_xor` method on [`BitSet`].
#[derive(Clone)]
pub struct BitXorIter<'a> {
    left_blocks: &'a [u64],
    right_blocks: &'a [u64],
    current_block: u64,
    current_block_idx: usize,
}

/// A lazy iterator yielding the difference (Bitwise AND NOT) of two bitsets.
///
/// This struct is created by the `iter_without` method on [`BitSet`].
#[derive(Clone)]
pub struct BitAndNotIter<'a> {
    left_blocks: &'a [u64],
    right_blocks: &'a [u64],
    current_block: u64,
    current_block_idx: usize,
}

impl<'a> FusedIterator for BitAndIter<'a> {}
impl<'a> FusedIterator for BitOrIter<'a> {}
impl<'a> FusedIterator for BitXorIter<'a> {}
impl<'a> FusedIterator for BitAndNotIter<'a> {}
impl<'a> FusedIterator for BitSetIter<'a> {}

impl<'a> Iterator for BitAndIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        while self.current_block == 0 {
            if self.current_block_idx >= self.left_blocks.len()
                || self.current_block_idx >= self.right_blocks.len()
            {
                return None;
            }

            let left_block = self.left_blocks[self.current_block_idx];
            let right_block = self.right_blocks[self.current_block_idx];

            self.current_block = left_block & right_block;
            self.current_block_idx += 1;
        }

        let bit_local_pos = self.current_block.trailing_zeros() as usize;
        let bit_global_pos =
            (64 * (self.current_block_idx - 1)) + bit_local_pos;

        // reset the analyzed bit
        self.current_block ^= 1u64 << bit_local_pos;

        Some(bit_global_pos)
    }
}

impl<'a> Iterator for BitOrIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        while self.current_block == 0 {
            if self.current_block_idx >= self.left_blocks.len()
                && self.current_block_idx >= self.right_blocks.len()
            {
                return None;
            }

            let left_block = self
                .left_blocks
                .get(self.current_block_idx)
                .copied()
                .unwrap_or(0) as u64;
            let right_block = self
                .right_blocks
                .get(self.current_block_idx)
                .copied()
                .unwrap_or(0);

            self.current_block = left_block | right_block;
            self.current_block_idx += 1;
        }

        let bit_local_pos = self.current_block.trailing_zeros() as usize;
        let bit_global_pos =
            (64 * (self.current_block_idx - 1)) + bit_local_pos;

        // reset the analyzed bit
        self.current_block ^= 1u64 << bit_local_pos;

        Some(bit_global_pos)
    }
}

impl<'a> Iterator for BitXorIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        while self.current_block == 0 {
            if self.current_block_idx >= self.left_blocks.len()
                && self.current_block_idx >= self.right_blocks.len()
            {
                return None;
            }

            let left_block = self
                .left_blocks
                .get(self.current_block_idx)
                .copied()
                .unwrap_or(0) as u64;
            let right_block = self
                .right_blocks
                .get(self.current_block_idx)
                .copied()
                .unwrap_or(0);

            self.current_block = left_block ^ right_block;
            self.current_block_idx += 1;
        }

        let bit_local_pos = self.current_block.trailing_zeros() as usize;
        let bit_global_pos =
            (64 * (self.current_block_idx - 1)) + bit_local_pos;

        // reset the analyzed bit
        self.current_block ^= 1u64 << bit_local_pos;

        Some(bit_global_pos)
    }
}

impl<'a> Iterator for BitAndNotIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        while self.current_block == 0 {
            if self.current_block_idx >= self.left_blocks.len() {
                return None;
            }

            let left_block = self.left_blocks[self.current_block_idx];
            let right_block = self
                .right_blocks
                .get(self.current_block_idx)
                .copied()
                .unwrap_or(0);

            self.current_block = left_block & (!right_block);
            self.current_block_idx += 1;
        }

        let bit_local_pos = self.current_block.trailing_zeros() as usize;
        let bit_global_pos =
            (64 * (self.current_block_idx - 1)) + bit_local_pos;

        // reset the analyzed bit
        self.current_block ^= 1u64 << bit_local_pos;

        Some(bit_global_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization_and_capacity() {
        let bitset = BitSet::with_capacity(100);
        assert_eq!(bitset.bit_blocks.len(), 2); // 100 bits requires two 64-bit blocks
        assert!(bitset.none());
    }

    #[test]
    fn test_set_and_test_boundaries() {
        let mut bitset = BitSet::default();

        // Test within first block
        bitset.set(0);
        bitset.set(63);
        assert!(bitset.test(0));
        assert!(bitset.test(63));
        assert!(!bitset.test(1)); // Unset bit

        // Test cross-block allocation (index 64 moves to the second block)
        assert!(!bitset.test(64));
        bitset.set(64);
        assert!(bitset.test(64));
    }

    #[test]
    fn test_reset_and_flip() {
        let mut bitset = BitSet::default();

        bitset.flip(10); // 0 -> 1
        assert!(bitset.test(10));

        bitset.flip(10); // 1 -> 0
        assert!(!bitset.test(10));

        bitset.set(5);
        bitset.reset(5);
        assert!(!bitset.test(5));

        // Resetting out of bounds should not panic
        bitset.reset(1000);
    }

    #[test]
    fn test_bitwise_not() {
        let mut a = BitSet::default();
        a.set(10); // first block
        a.set(20);
        a.set(100); // second block

        let result = !a;

        assert!(result.test(0)); // was flipped to (1)
        assert!(!result.test(10));
        assert!(!result.test(20));
        assert!(!result.test(100));

        assert_eq!(result.count(), 125); // 128 - 3 = 125 bits
    }

    #[test]
    fn test_bitwise_and() {
        let mut a = BitSet::default();
        a.set(10);
        a.set(20);
        a.set(100); // 100 is in block 1

        let mut b = BitSet::default();
        b.set(20);
        b.set(30);
        // b only goes up to block 0

        let result = a & b;

        assert!(!result.test(10));
        assert!(result.test(20)); // Only 20 is present in both
        assert!(!result.test(30));
        assert!(!result.test(100)); // Truncation logic should handle block mismatches safely

        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_bitwise_or() {
        let mut a = BitSet::default();
        a.set(10);

        let mut b = BitSet::default();
        b.set(100); // Forces allocation difference

        let result = a | b;

        assert!(result.test(10));
        assert!(result.test(100));

        assert_eq!(result.count(), 2);
    }

    #[test]
    fn test_bitwise_xor() {
        let mut a = BitSet::default();
        a.set(10);
        a.set(0);

        let mut b = BitSet::default();
        b.set(100); // Forces allocation difference
        b.set(10);

        let result = a ^ b;

        assert!(result.test(0));
        assert!(!result.test(10));
        assert!(result.test(100));

        assert_eq!(result.count(), 2);
    }

    #[test]
    fn test_from_bytes() {
        // [00000001, 00000010] (Little-endian layout evaluation)
        let bytes: &[u8] = &[1, 2];
        let bitset = BitSet::from(bytes);

        assert!(bitset.test(0)); // First bit of first byte
        assert!(bitset.test(9)); // Second bit of second byte
    }

    #[test]
    fn test_count_bits_1() {
        let bitset = BitSet::new(&[0b01110001]); // four bits set (1)
        assert_eq!(bitset.count(), 4);

        let bitset = BitSet::default(); // no bits set (1)
        assert_eq!(bitset.count(), 0)
    }

    #[test]
    fn test_iterator_empty() {
        let bitset = BitSet::default();
        let result: Vec<usize> = bitset.iter().into_iter().collect();
        assert!(result.is_empty(), "A empty BitSet has no elements.");
    }

    #[test]
    fn test_iterator_single_block() {
        let mut bitset = BitSet::default();

        bitset.set(0);
        bitset.set(42);
        bitset.set(63);

        let result: Vec<usize> = bitset.iter().into_iter().collect();
        assert_eq!(
            result,
            vec![0, 42, 63],
            "The iterator failed to process the block boundaries"
        );
    }

    #[test]
    fn test_iterator_multiple_blocks_with_gaps() {
        let mut bitset = BitSet::default();

        bitset.set(5); // block 0
        bitset.set(64); // block 1 
        bitset.set(150); // block 2
        bitset.set(400); // block 6

        let result: Vec<usize> = bitset.iter().into_iter().collect();
        assert_eq!(
            result,
            vec![5, 64, 150, 400],
            "The iterator did not jumped the empty blocks."
        );
    }

    #[test]
    fn test_iterator_dense_sequence() {
        let mut bitset = BitSet::default();

        // sequentially fill between blocks
        for i in 60..70 {
            bitset.set(i);
        }

        let result: Vec<usize> = bitset.iter().into_iter().collect();
        let expected: Vec<usize> = (60..70).collect();

        assert_eq!(
            result, expected,
            "The iterator lost data between dense blocks."
        );
    }
    #[test]
    fn test_lazy_iterators_offset() {
        let mut a = BitSet::default();
        let mut b = BitSet::default();

        // first block
        a.set(5);
        // second block
        a.set(70);

        b.set(5);
        b.set(70);

        let result: Vec<usize> = a.iter_and(&b).collect();

        assert_eq!(
            result,
            vec![5, 70],
            "The iterator failed in block aligment and returned wrong global positions."
        );
    }

    #[test]
    fn test_lazy_iterator_without() {
        let mut a = BitSet::default();
        let mut exclude = BitSet::default();

        a.set(10);
        a.set(20);
        a.set(100);

        exclude.set(20);

        let result: Vec<usize> = a.iter_without(&exclude).collect();
        assert_eq!(
            result,
            vec![10, 100],
            "The exclude does not correctly perfomed or shift the IDs."
        );
    }

    #[test]
    fn test_bitwise_or_active_count() {
        let mut a = BitSet::default();
        a.set(10); // first block

        let mut b = BitSet::default();
        b.set(100); // second block

        let result = &a | &b;

        assert_eq!(
            result.count(),
            2,
            "The BitOr failed to calculate the active_counte on excedents blocks."
        );
        assert!(result.test(10) && result.test(100));
    }

    #[test]
    fn test_bitwise_and_active_count() {
        let mut a = BitSet::default();
        a.set(10);
        a.set(20);

        let mut b = BitSet::default();
        b.set(10);
        b.set(30);

        let result = &a & &b;

        assert_eq!(
            result.count(),
            1,
            "BitAnd was not correctly updated the active_count."
        );
    }

    #[test]
    fn test_count_synchronization_on_flips() {
        let mut bitset = BitSet::with_capacity(128); // 2 blocks

        bitset.set(0);
        bitset.set(64);
        assert_eq!(bitset.count(), 2);

        bitset.flip(0);
        assert_eq!(bitset.count(), 1);

        bitset.flip_all();
        assert_eq!(
            bitset.count(),
            127,
            "The active_count failed to synchronize when flip all."
        );
    }
}
