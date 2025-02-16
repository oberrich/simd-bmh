#![no_std]
#![feature(portable_simd)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![feature(maybe_uninit_uninit_array)]

extern crate alloc;
use crate::alloc::vec::Vec;

use core::simd::{Simd, cmp::SimdPartialEq, LaneCount, SupportedLaneCount};
use core::ops::{BitAnd, BitAndAssign};
use core::arch::x86_64::*;
pub use simd_bmh_macro::parse_pattern;

#[derive(Clone, Debug)]
#[repr(align(32))]
pub struct Pattern<const N: usize> {
    pub bytes: [u8; N],
    pub masks: [u8; N],
    pub best_skip_value: u8,
    pub best_skip_mask: u8,
    pub max_skip: usize,
    pub best_skip_offset: usize,
}

impl<const N: usize> Pattern<N> {
    #[inline(always)]
    pub fn find_all_matches(&self, text: &[u8]) -> Vec<usize> {
        find_all_matches_sse::<N>(text, self)
    }
}

#[inline(always)]
pub fn find_all_matches_sse<const PATTERN_LEN: usize>(text: &[u8], pattern: &Pattern<PATTERN_LEN>) -> Vec<usize> {
    if PATTERN_LEN > text.len() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let mut i = 0;

    let best_skip = pattern.best_skip_value as i32;
    let best_mask = pattern.best_skip_mask as i32;
    let best_skip_offset = pattern.best_skip_offset as i32;

    unsafe {
        let skip_vector = _mm_set1_epi8(best_skip as i8);
        let mask_vector = _mm_set1_epi8(best_mask as i8);

        while i + 16 <= text.len() {
            let mut match_masks = _mm_setzero_si128();
            let chunk = _mm_loadu_si128(text.as_ptr().add(i) as *const __m128i);
            let masked_chunk = _mm_and_si128(chunk, mask_vector);
            let cmp_result = _mm_cmpeq_epi8(masked_chunk, skip_vector);
            match_masks = _mm_or_si128(match_masks, cmp_result);

            let match_positions = _mm_movemask_epi8(match_masks);
            if match_positions != 0 {
                for pos in 0..16 {
                    if (match_positions & (1 << pos)) != 0 {
                        let match_pos = i + pos;
                        let start_pos = match_pos - best_skip_offset as usize;
                        
                        let mut valid = true;
                        for k in 0..PATTERN_LEN {
                            let pattern_byte = pattern.bytes[k];
                            let pattern_mask = pattern.masks[k];
                            let text_index = start_pos + k;

                            let masked_pattern_byte = pattern_byte & pattern_mask;
                            let masked_text_byte = text[text_index] & pattern_mask;
                            if masked_text_byte != masked_pattern_byte {
                                valid = false;
                                break;
                            }
                        }

                        if valid {
                            matches.push(start_pos);
                        }
                    }
                }
            }

            i += 16;
        }
    }

    while i + PATTERN_LEN <= text.len() {
        let start_pos = i;
        let mut match_found = true;

        for k in 0..PATTERN_LEN {
            let pattern_byte = pattern.bytes[k];
            let pattern_mask = pattern.masks[k];
            let text_index = start_pos + k;

            let masked_pattern_byte = pattern_byte & pattern_mask;
            let masked_text_byte = text[text_index] & pattern_mask;

            if masked_text_byte != masked_pattern_byte {
                match_found = false;
                break;
            }
        }

        if match_found {
            matches.push(start_pos);
            i += PATTERN_LEN;
        } else {
            let mismatch_byte = text[start_pos + PATTERN_LEN - 1];
            i += (0..PATTERN_LEN - 1)
                .rev()
                .find(|&j| pattern.bytes[j] == mismatch_byte)
                .map_or(PATTERN_LEN, |j| PATTERN_LEN - 1 - j);
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use rand::Rng;

    #[test]
    fn test_parse_pattern() {
        let pattern = parse_pattern!("A?C?FF");
        assert_eq!(&pattern.bytes[..], &[0xA0, 0xC0, 0xFF]);
        assert_eq!(&pattern.masks[..], &[0xF0, 0xF0, 0xFF]);
    }

    #[test]
    fn test_match() {
        let pattern = parse_pattern!("A?C?FF");
        let text = b"\xA0\xC0\xFF\x00\xA0\xC0\xFF";

        let matches = pattern.find_all_matches(text);
        assert_eq!(matches, [0, 4]);
    }
    
    #[test]
    fn test_random_pool_with_fixed_pattern() {
        let buffer_size = 2_000;
        let mut random_buffer: Vec<u8> = (0..buffer_size).map(|_| rand::rng().random()).collect();
        random_buffer[1337..1342].copy_from_slice(b"\xAA\xCC\xFF\xFF\xFF");

        let pattern = parse_pattern!("A?C?FF");
        let matches = find_all_matches_sse(&random_buffer, &pattern);
        assert!(!matches.is_empty(), "Pattern matches should not be empty!");
    }
}