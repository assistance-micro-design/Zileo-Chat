// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Tests for the recursive chunker.
//!
//! These tests pin down the contract of `split_recursive`:
//! - Short text passes through untouched.
//! - Splits prefer larger semantic boundaries first
//!   (`\n\n` > `\n` > sentence punctuation > hard cut).
//! - Hard cuts respect UTF-8 char boundaries (never split a code point).
//! - `overlap = 0` yields disjoint chunks, `overlap > 0` shares chars
//!   between consecutive chunks.
//! - Every produced chunk fits within `chunk_size` chars.

use super::{split_recursive, DEFAULT_CHUNK_OVERLAP, DEFAULT_CHUNK_SIZE};

#[test]
fn split_recursive_empty_text_returns_empty_vec() {
    let out = split_recursive("", 100, 10);
    assert!(out.is_empty(), "empty input must produce no chunks");
}

#[test]
fn split_recursive_short_text_returns_single_chunk() {
    let text = "hello world";
    let out = split_recursive(text, 100, 10);
    assert_eq!(out, vec![text.to_string()]);
}

#[test]
fn split_recursive_exactly_chunk_size_returns_single() {
    // 100 ASCII chars = 100 unicode chars => single chunk
    let text = "x".repeat(100);
    let out = split_recursive(&text, 100, 10);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0], text);
}

#[test]
fn split_recursive_paragraph_breaks_preferred() {
    // Two paragraphs of 60 chars each separated by `\n\n`.
    // With chunk_size=80 and overlap=0, each paragraph must end up in
    // its own chunk (no mixing across the boundary).
    let p1 = "a".repeat(60);
    let p2 = "b".repeat(60);
    let text = format!("{}\n\n{}", p1, p2);
    let out = split_recursive(&text, 80, 0);
    assert_eq!(out.len(), 2, "got: {:?}", out);
    assert!(out[0].contains('a') && !out[0].contains('b'));
    assert!(out[1].contains('b') && !out[1].contains('a'));
}

#[test]
fn split_recursive_line_breaks_secondary() {
    // One paragraph with 4 lines of 30 chars each => 4*30 + 3 newlines = 123 chars.
    // With chunk_size=70 and overlap=0, the chunker should split on `\n`
    // (no `\n\n` available), producing chunks each containing whole lines.
    let line = "a".repeat(30);
    let text = format!("{}\n{}\n{}\n{}", line, line, line, line);
    let out = split_recursive(&text, 70, 0);
    assert!(out.len() >= 2, "expected splitting, got: {:?}", out);
    for c in &out {
        assert!(
            c.chars().count() <= 70,
            "chunk exceeds size limit: {:?} ({} chars)",
            c,
            c.chars().count()
        );
    }
}

#[test]
fn split_recursive_sentence_breaks_tertiary() {
    // Single line (no \n, no \n\n) with two sentences > chunk_size.
    // Splitter should fall back to sentence boundaries (`. `).
    let s1 = format!("{}.", "a".repeat(60));
    let s2 = format!("{}.", "b".repeat(60));
    let text = format!("{} {}", s1, s2);
    let out = split_recursive(&text, 80, 0);
    assert!(out.len() >= 2, "expected sentence split, got: {:?}", out);
    for c in &out {
        assert!(c.chars().count() <= 80);
    }
}

#[test]
fn split_recursive_hard_cut_utf8_safe_french_accents() {
    // Single token, no separators at all => forced hard cut. Verify UTF-8
    // safety on multi-byte chars (each 'é' is 2 bytes / 1 char).
    let text = "é".repeat(100);
    let out = split_recursive(&text, 30, 0);
    assert!(out.len() >= 3);
    for c in &out {
        // Must roundtrip cleanly => no broken UTF-8 sequences
        assert!(c.chars().all(|ch| ch == 'é'));
        assert!(c.chars().count() <= 30);
    }
}

#[test]
fn split_recursive_hard_cut_utf8_safe_emoji() {
    // Emojis are 4-byte UTF-8 sequences. Naive byte slicing on a 4-byte
    // boundary panics — this test pins the char-boundary safety.
    let text = "🚀".repeat(20);
    let out = split_recursive(&text, 5, 0);
    assert!(out.len() >= 4);
    for c in &out {
        assert!(c.chars().all(|ch| ch == '🚀'));
        assert!(c.chars().count() <= 5);
    }
}

#[test]
fn split_recursive_overlap_zero_disjoint() {
    // With overlap=0, consecutive chunks must NOT share any prefix/suffix
    // (modulo separator whitespace which the chunker strips at boundaries).
    let line = "a".repeat(30);
    let text = format!("{}\n{}\n{}", line, line, line);
    let out = split_recursive(&text, 35, 0);
    assert!(out.len() >= 2);
    let total_chars: usize = out.iter().map(|c| c.chars().count()).sum();
    // With overlap=0, total chars in chunks must be <= total chars in input
    // (separators are dropped, never duplicated).
    assert!(total_chars <= text.chars().count());
}

#[test]
fn split_recursive_overlap_carries_tail_into_next_chunk() {
    // With overlap > 0 and chunk_size set so the input requires several
    // chunks, consecutive chunks must share at least one character at the
    // boundary (the overlap tail).
    let text = "a".repeat(200);
    let chunk_size = 50;
    let overlap = 10;
    let out = split_recursive(&text, chunk_size, overlap);
    assert!(out.len() >= 3, "got: {:?}", out);
    // The total char count across chunks must EXCEED the input length
    // when overlap > 0 (duplicated tail).
    let total_chars: usize = out.iter().map(|c| c.chars().count()).sum();
    assert!(
        total_chars > text.chars().count(),
        "expected overlap to duplicate chars, total={} input={}",
        total_chars,
        text.chars().count()
    );
}

#[test]
fn split_recursive_each_chunk_within_chunk_size() {
    // Invariant: every chunk has at most `chunk_size` chars, regardless of
    // input pathology.
    let text = format!(
        "{}\n\n{}\n{}. {}",
        "a".repeat(100),
        "b".repeat(80),
        "c".repeat(60),
        "d".repeat(120)
    );
    let chunk_size = 50;
    let out = split_recursive(&text, chunk_size, 5);
    for c in &out {
        assert!(
            c.chars().count() <= chunk_size,
            "chunk exceeds size limit: {} chars (max {})",
            c.chars().count(),
            chunk_size
        );
    }
}

#[test]
fn split_recursive_defaults_are_in_expected_range() {
    // Pin the constants used by the production path so unrelated bumps
    // are loud. The `< chunk_size` invariant is enforced at compile time
    // by the `const _: () = assert!(...)` block in chunker.rs.
    assert_eq!(DEFAULT_CHUNK_SIZE, 512);
    assert_eq!(DEFAULT_CHUNK_OVERLAP, 50);
}
