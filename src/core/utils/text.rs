use std::collections::BTreeSet;

use crate::PagerState;

pub enum AppendStyle {
    PartialUpdate((Vec<String>, usize)),
    FullRedraw,
}

pub struct AppendProps {
    pub lines: Vec<String>,
    pub num_unterminated: usize,
    #[cfg(feature = "search")]
    pub append_search_idx: BTreeSet<usize>,
}

/// Makes the text that will be displayed and appended it to [`self.formatted_lines`]
///
/// - The first output value is the actual text rows that needs to be appended. This is wrapped
///     based on the available columns
/// - The second value is the number of rows that should be truncated from [`self.formatted_lines`]
///     before appending this line. This will be 0 if the given `text` is to be appended to
///     [`self.formatted_lines`] but will be `>0` if the given text is actually part of the
///     last appended line. This function determines this by checking whether self.lines ends with
///     `\n` after appending the text
pub fn make_append_str(
    p: &PagerState,
    text: &str,
    attachment: Option<String>,
    to_skip: usize,
    len_line_number: usize,
) -> AppendProps {
    let append;
    let to_format = if let Some(attached_text) = attachment {
        let mut s = String::with_capacity(text.len() + attached_text.len());
        s.push_str(&attached_text);
        s.push_str(text);
        append = false;
        s
    } else {
        append = true;
        text.to_string()
    };

    // This will get filled if there is an ongoing search. We just need to append it to
    // self.search_idx at the end
    #[cfg(feature = "search")]
    let mut append_search_idx = BTreeSet::new();

    // If append is true, we take only the given text for formatting
    // else we also take the last line of self.lines for formatting. This is because we nned to
    // format the entire line rathar than just this part
    let to_format_len = to_format.lines().count();
    let lines = to_format
        .lines()
        .enumerate()
        .map(|(idx, s)| (idx, s.to_string()))
        .collect::<Vec<(usize, String)>>();

    let mut fmtl = Vec::with_capacity(256);

    // First line
    let mut first_line = p.formatted_line(
        // TODO: Remove unwrap from here
        &lines.first().unwrap().1,
        len_line_number,
        to_skip.saturating_sub(1),
        #[cfg(feature = "search")]
        if append {
            to_skip
        } else {
            to_skip.saturating_sub(1)
        },
        #[cfg(feature = "search")]
        &mut append_search_idx,
    );

    // Format the last line, only if first line and last line are different. We can check this
    // by seeing whether to_format_len is greater than 1
    let last_line = if to_format_len > 1 {
        Some(p.formatted_line(
            &lines.last().unwrap().1,
            len_line_number,
            to_format_len + to_skip.saturating_sub(1),
            #[cfg(feature = "search")]
            to_skip,
            #[cfg(feature = "search")]
            &mut append_search_idx,
        ))
    } else {
        None
    };

    // Format all other lines except the first and last line
    let mut mid_lines = lines
        .iter()
        .skip(1)
        .take(lines.len().saturating_sub(2))
        .flat_map(|(idx, line)| {
            p.formatted_line(
                line,
                len_line_number,
                idx + to_skip.saturating_sub(1),
                #[cfg(feature = "search")]
                to_skip,
                #[cfg(feature = "search")]
                &mut append_search_idx,
            )
        })
        .collect::<Vec<String>>();

    let unterminated = if text.ends_with('\n') {
        0
    } else if to_format_len > 1 {
        last_line.as_ref().unwrap().len()
    } else {
        first_line.len()
    };

    fmtl.append(&mut first_line);
    fmtl.append(&mut mid_lines);
    if let Some(mut ll) = last_line {
        fmtl.append(&mut ll);
    }

    AppendProps {
        lines: fmtl,
        num_unterminated: unterminated,
        #[cfg(feature = "search")]
        append_search_idx,
    }
}