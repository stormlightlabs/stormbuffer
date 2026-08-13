use crate::{Embedder, Error, Record};

use super::MAX_CHUNK_WORDS;

pub fn chunk_record(record: &Record) -> Vec<(String, Option<String>, String, usize)> {
    let mut sections = Vec::new();
    let mut heading_stack: Vec<(usize, String)> = Vec::new();
    let mut current_lines: Vec<&str> = Vec::new();
    let mut current_atomic = false;
    let mut in_fence: Option<char> = None;

    for line in record.body.lines() {
        let trimmed = line.trim_start();
        if let Some(fence) = in_fence {
            current_lines.push(line);
            if is_fence_end(trimmed, fence) {
                in_fence = None;
            }
            continue;
        }

        if let Some(fence) = fence_start(trimmed) {
            flush_section(
                &mut sections,
                &mut current_lines,
                &heading_stack,
                &mut current_atomic,
            );
            current_lines.push(line);
            current_atomic = true;
            in_fence = Some(fence);
            continue;
        }

        if let Some((level, heading)) = parse_heading(trimmed) {
            flush_section(
                &mut sections,
                &mut current_lines,
                &heading_stack,
                &mut current_atomic,
            );
            while heading_stack
                .last()
                .is_some_and(|(previous, _)| *previous >= level)
            {
                heading_stack.pop();
            }
            heading_stack.push((level, heading));
            continue;
        }

        if line.trim().is_empty() {
            flush_section(
                &mut sections,
                &mut current_lines,
                &heading_stack,
                &mut current_atomic,
            );
            continue;
        }

        if is_list_start(trimmed) && !current_atomic {
            flush_section(
                &mut sections,
                &mut current_lines,
                &heading_stack,
                &mut current_atomic,
            );
            current_atomic = true;
        }
        current_lines.push(line);
    }
    flush_section(
        &mut sections,
        &mut current_lines,
        &heading_stack,
        &mut current_atomic,
    );

    if sections.is_empty() {
        let heading = heading_stack
            .iter()
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        sections.push((
            heading.clone(),
            (!heading.is_empty()).then_some(heading),
            String::new(),
            false,
        ));
    }

    let mut chunks = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_text = String::new();
    let mut current_words = 0;

    let push_chunk = |chunks: &mut Vec<(String, Option<String>, String, usize)>,
                      heading: &mut Option<String>,
                      text: &mut String,
                      words: &mut usize| {
        if text.trim().is_empty() {
            return;
        }
        let ordinal = chunks.len();
        chunks.push((
            format!("{}:{ordinal}", record.id),
            heading.clone(),
            text.clone(),
            *words,
        ));
        text.clear();
        *words = 0;
    };

    for (_, heading, text, atomic) in sections {
        let word_count = text.split_whitespace().count();
        if atomic {
            push_chunk(
                &mut chunks,
                &mut current_heading,
                &mut current_text,
                &mut current_words,
            );
            current_heading = heading.clone();
            current_text = text;
            current_words = word_count;
            push_chunk(
                &mut chunks,
                &mut current_heading,
                &mut current_text,
                &mut current_words,
            );
            continue;
        }

        if word_count > MAX_CHUNK_WORDS {
            push_chunk(
                &mut chunks,
                &mut current_heading,
                &mut current_text,
                &mut current_words,
            );
            let words: Vec<_> = text.split_whitespace().collect();
            for piece in words.chunks(MAX_CHUNK_WORDS) {
                let piece_text = piece.join(" ");
                chunks.push((
                    format!("{}:{}", record.id, chunks.len()),
                    heading.clone(),
                    piece_text,
                    piece.len(),
                ));
            }
            continue;
        }

        let same_heading = current_heading == heading;
        let separator_words = usize::from(!current_text.is_empty());
        if !same_heading || current_words + separator_words + word_count > MAX_CHUNK_WORDS {
            push_chunk(
                &mut chunks,
                &mut current_heading,
                &mut current_text,
                &mut current_words,
            );
            current_heading = heading.clone();
        }
        if !current_text.is_empty() {
            current_text.push_str("\n\n");
            current_words += 1;
        }
        current_text.push_str(&text);
        current_words += word_count;
    }
    push_chunk(
        &mut chunks,
        &mut current_heading,
        &mut current_text,
        &mut current_words,
    );

    chunks
}

fn flush_section(
    sections: &mut Vec<(String, Option<String>, String, bool)>,
    current_lines: &mut Vec<&str>,
    heading_stack: &[(usize, String)],
    current_atomic: &mut bool,
) {
    if current_lines.is_empty() {
        return;
    }
    let heading = heading_stack
        .iter()
        .map(|(_, value)| value.as_str())
        .collect::<Vec<_>>()
        .join(" / ");
    sections.push((
        heading.clone(),
        (!heading.is_empty()).then_some(heading),
        current_lines.join("\n"),
        *current_atomic,
    ));
    current_lines.clear();
    *current_atomic = false;
}

pub(super) fn split_embedding_text(
    text: &str,
    embedder: &dyn Embedder,
) -> crate::Result<Vec<String>> {
    if embedder.token_count(text)? <= embedder.max_tokens() {
        return Ok(vec![text.to_owned()]);
    }

    let mut pieces = Vec::new();
    let mut remaining = text.trim();
    while !remaining.is_empty() {
        // Bound each exact-tokenizer probe. This keeps splitting linear in the
        // input size even for long identifiers while allowing ample room for
        // tokenizers that merge many adjacent characters.
        let candidate_chars = embedder.max_tokens().saturating_mul(16).max(1);
        let candidate_end = remaining
            .char_indices()
            .nth(candidate_chars)
            .map_or(remaining.len(), |(offset, _)| offset);
        let candidate = &remaining[..candidate_end];
        let boundaries = candidate
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(candidate.len()))
            .collect::<Vec<_>>();
        let mut low = 1usize;
        let mut high = boundaries.len().saturating_sub(1);
        let mut fitting = None;
        while low <= high {
            let middle = low + (high - low) / 2;
            let end = boundaries[middle];
            if embedder.token_count(&candidate[..end])? <= embedder.max_tokens() {
                fitting = Some(end);
                low = middle + 1;
            } else {
                high = middle.saturating_sub(1);
            }
        }
        let maximum = fitting.ok_or_else(|| {
            Error::embedding(
                "chunk record for embedding",
                format!(
                    "one content character cannot fit within the {}-token model limit",
                    embedder.max_tokens()
                ),
            )
        })?;
        let preferred = remaining[..maximum]
            .char_indices()
            .rev()
            .find(|(_, character)| character.is_whitespace())
            .map(|(offset, character)| offset + character.len_utf8())
            .filter(|end| *end > 0)
            .unwrap_or(maximum);
        pieces.push(remaining[..preferred].trim_end().to_owned());
        remaining = remaining[preferred..].trim_start();
    }
    Ok(pieces)
}

pub(super) fn retrieval_text(record: &Record, heading: &str, filename: &str, text: &str) -> String {
    [
        record.title.as_str(),
        heading,
        record.aliases.join(" ").as_str(),
        record.tags.join(" ").as_str(),
        filename,
        text,
    ]
    .join("\n")
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(1..=6).contains(&level) || !line.chars().nth(level).is_some_and(char::is_whitespace) {
        return None;
    }
    let heading = line[level..].trim().trim_end_matches('#').trim();
    (!heading.is_empty()).then(|| (level, heading.to_owned()))
}

fn fence_start(line: &str) -> Option<char> {
    if line.starts_with("```") {
        Some('`')
    } else if line.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

fn is_fence_end(line: &str, fence: char) -> bool {
    line.chars()
        .take_while(|character| *character == fence)
        .count()
        >= 3
}

fn is_list_start(line: &str) -> bool {
    line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("+ ")
        || line
            .split_once(' ')
            .is_some_and(|(prefix, _)| !prefix.is_empty() && prefix.ends_with('.'))
}
