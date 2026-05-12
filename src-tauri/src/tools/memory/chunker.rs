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

//! Recursive text chunker for embedding indexing.
//!
//! [`split_recursive`] is a pure function with no I/O — given a string and a
//! target chunk size + overlap, it returns a list of substrings, each no
//! larger than `chunk_size` chars, that together cover the input.
//!
//! ## Strategy
//!
//! Splits are attempted in priority order on increasingly fine separators:
//! 1. paragraph (`\n\n`) — keeps top-level structure intact
//! 2. line (`\n`) — keeps individual lines together
//! 3. sentence (`. `, `! `, `? `) — keeps sentences together
//! 4. hard cut on char boundary (UTF-8 safe via `chars().take(N)`)
//!
//! After splitting into atomic pieces, consecutive pieces are merged until
//! the accumulated length would exceed `chunk_size`. When `overlap > 0`, the
//! tail of each chunk is duplicated as the start of the next (semantic
//! continuity for vector search).
//!
//! The function never panics on valid UTF-8 input: hard cuts walk char
//! indices, not byte indices.

/// Default size of each chunk in characters.
///
/// 512 chars ≈ ~100-150 tokens for typical English/French prose, comfortably
/// below the per-input limits of the supported embedding providers
/// (Mistral 8192 tokens, Ollama mxbai 512 tokens).
pub const DEFAULT_CHUNK_SIZE: usize = 512;

/// Default character overlap between consecutive chunks.
///
/// 50 chars ≈ 10% of `DEFAULT_CHUNK_SIZE`, enough to keep sentence-level
/// context across the boundary without significant duplication cost.
pub const DEFAULT_CHUNK_OVERLAP: usize = 50;

// Compile-time invariant: overlap must leave room for new content.
const _: () = assert!(DEFAULT_CHUNK_OVERLAP < DEFAULT_CHUNK_SIZE);

/// Ordered list of separators tried by the recursive splitter, from largest
/// semantic scope (paragraph break) to smallest (sentence terminator).
const SEPARATORS: &[&str] = &["\n\n", "\n", ". ", "! ", "? "];

/// Splits text into overlapping chunks using a recursive strategy.
///
/// # Arguments
/// * `text` - The input string to chunk
/// * `chunk_size` - Maximum number of *characters* (not bytes) per chunk
/// * `overlap` - Number of characters duplicated between consecutive chunks
///
/// # Returns
/// A list of chunks. Empty input returns an empty vec. Short input
/// (≤ `chunk_size` chars) returns a single-element vec containing the input.
///
/// # Guarantees
/// - Every chunk has at most `chunk_size` chars.
/// - Concatenating chunks (without their overlaps) reconstructs the input up
///   to separator whitespace that the splitter consumes at chunk boundaries.
/// - Never panics on any UTF-8 input.
pub fn split_recursive(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    if text.chars().count() <= chunk_size {
        return vec![text.to_string()];
    }

    // Reserve space in each atomic piece for the overlap seed (carried from
    // the previous chunk) plus a 1-char join separator. Without this,
    // pieces sized exactly `chunk_size` chars would force `merge_with_overlap`
    // to drop the seed at every boundary and overlap would be invisible.
    let atomic_max = chunk_size.saturating_sub(overlap).saturating_sub(1).max(1);
    let pieces = split_atomic(text, atomic_max, 0);
    merge_with_overlap(pieces, chunk_size, overlap)
}

/// Recursively splits `text` into pieces each no larger than `chunk_size`
/// chars, descending through `SEPARATORS` and falling back to a hard cut.
fn split_atomic(text: &str, chunk_size: usize, sep_idx: usize) -> Vec<String> {
    if text.chars().count() <= chunk_size {
        return if text.is_empty() {
            Vec::new()
        } else {
            vec![text.to_string()]
        };
    }
    if sep_idx >= SEPARATORS.len() {
        return hard_cut(text, chunk_size);
    }

    let sep = SEPARATORS[sep_idx];
    if !text.contains(sep) {
        // No occurrence: skip to the next finer separator.
        return split_atomic(text, chunk_size, sep_idx + 1);
    }

    let mut out: Vec<String> = Vec::new();
    for part in text.split(sep) {
        if part.is_empty() {
            continue;
        }
        if part.chars().count() <= chunk_size {
            out.push(part.to_string());
        } else {
            out.extend(split_atomic(part, chunk_size, sep_idx + 1));
        }
    }
    out
}

/// UTF-8 safe hard cut: builds slices by char index, never by byte index.
fn hard_cut(text: &str, chunk_size: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::with_capacity(chars.len().div_ceil(chunk_size.max(1)));
    let mut i = 0;
    while i < chars.len() {
        let end = (i + chunk_size).min(chars.len());
        out.push(chars[i..end].iter().collect());
        i = end;
    }
    out
}

/// Merges atomic pieces into chunks of at most `chunk_size` chars, applying
/// the requested overlap between consecutive chunks.
///
/// Overlap is implemented by carrying the last `overlap` chars of the just-
/// flushed chunk as the seed of the next one. Pieces individually larger
/// than `chunk_size` are unreachable here (caller guarantees atomic pieces
/// are bounded) but a defensive hard cut keeps the function total.
fn merge_with_overlap(pieces: Vec<String>, chunk_size: usize, overlap: usize) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_count: usize = 0;
    let overlap = overlap.min(chunk_size.saturating_sub(1));

    for piece in pieces {
        let piece_count = piece.chars().count();
        // Pieces should already be <= chunk_size (caller guarantee), but if
        // not, hard-cut defensively and process each fragment as a piece.
        if piece_count > chunk_size {
            let frags = hard_cut(&piece, chunk_size);
            for frag in frags {
                push_piece(
                    frag,
                    chunk_size,
                    overlap,
                    &mut chunks,
                    &mut current,
                    &mut current_count,
                );
            }
            continue;
        }
        push_piece(
            piece,
            chunk_size,
            overlap,
            &mut chunks,
            &mut current,
            &mut current_count,
        );
    }

    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Appends a single piece to `current`, flushing into `chunks` when adding
/// it would overflow `chunk_size`. After a flush, the last `overlap` chars
/// of the flushed chunk seed the new current.
fn push_piece(
    piece: String,
    chunk_size: usize,
    overlap: usize,
    chunks: &mut Vec<String>,
    current: &mut String,
    current_count: &mut usize,
) {
    let piece_count = piece.chars().count();
    // +1 for the separator we insert between joined pieces (only when current is non-empty).
    let join_cost = if current.is_empty() { 0 } else { 1 };

    if *current_count + join_cost + piece_count <= chunk_size {
        if join_cost == 1 {
            current.push(' ');
            *current_count += 1;
        }
        current.push_str(&piece);
        *current_count += piece_count;
        return;
    }

    // Flush current and seed next chunk with overlap tail.
    if !current.is_empty() {
        chunks.push(current.clone());
        if overlap > 0 && *current_count > 0 {
            let chars: Vec<char> = current.chars().collect();
            let tail_start = chars.len().saturating_sub(overlap);
            let tail: String = chars[tail_start..].iter().collect();
            *current = tail;
            *current_count = current.chars().count();
        } else {
            current.clear();
            *current_count = 0;
        }
    }

    // After flush, if the piece itself fits the next chunk along with the
    // overlap seed, append it; otherwise start fresh from the piece.
    if !current.is_empty() && *current_count + 1 + piece_count <= chunk_size {
        current.push(' ');
        *current_count += 1;
        current.push_str(&piece);
        *current_count += piece_count;
    } else if piece_count <= chunk_size {
        // The piece does not fit alongside the overlap seed → drop the seed
        // and start the next chunk cleanly with this piece. Overlap is a
        // best-effort hint, not a hard guarantee on every boundary.
        current.clear();
        current.push_str(&piece);
        *current_count = piece_count;
    } else {
        // Should be unreachable (caller bounds pieces) — defensive only.
        current.clear();
        *current_count = 0;
        for frag in hard_cut(&piece, chunk_size) {
            chunks.push(frag);
        }
    }
}

#[cfg(test)]
#[path = "chunker_tests.rs"]
mod tests;
