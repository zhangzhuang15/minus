//! Text related functions
//!
//! minus has a very interesting but simple text model that you must go through to understand how minus works.
//!
//! # Text Block
//! A text block in minus is just a bunch of text that may contain newlines (`\n`) between them.
//! [`PagerState::lines`] is nothing but just a giant text block.
//!
//! # Line
//! A line is text that must not contain any newlines inside it but may or may not end with a newline.
//! Don't confuse this with Rust's [Lines](std::str::Lines) which is similar to minus's Lines terminolagy but only
//! differs for the fact that they don't end with a newline. Although the Rust's Lines is heavily used inside minus
//! as an important building block.
//!
//! # Row
//! A row is part of a line that fits perfectly inside one row of terminal. Out of the three text types, only row
//! is dependent on the terminal conditions. If the terminal gets resized, each row will grow or shrink to hold
//! more or less text inside it.
//!
//! # Termination
//! # Termination of Line
//! A line is called terminated when it ends with a newline character, otherwise it is called unterminated.
//! You may ask why is this important? Because minus supports completing a line in multiple steps, if we don't care
//! whether a line is terminated or not, we won't know that the data coming right now is part of the current line or
//! it is for a new line.
//!
//! # Termination of block
//! A block is terminated if the last line of the block is terminated i.e it ends with a newline character.
//!
//! # Unterminated rows
//! It is 0 in most of the cases. The only case when it has a non-zero value is a line or block of text is unterminated
//! In this case, it is equal to the number of rows that the last line of the block or a the line occupied.
//!
//! Whenever new data comes while a line or block is unterminated minus cleans up the number of unterminated rows
//! on the terminal i.e the entire last line. Then it merges the incoming data to the last line and then reprints
//! them on the terminal.
//!
//! Why this complex approach?  
//! Simple! printing an entire page on the terminal is slow and this approach allows minus to reprint only the
//! parts that are required without having to redraw everything
//!
//! [`PagerState::lines`]: crate::state::PagerState::lines

use std::borrow::Cow;

use crate::{minus_core, LineNumbers};

use super::LinesRowMap;

#[cfg(feature = "search")]
use {crate::search, std::collections::BTreeSet};

/// How should the incoming text be drawn on the screen
pub enum AppendStyle {
    /// Draw only the region that needs to change
    PartialUpdate(Vec<String>),

    /// Redraw the entire screen
    FullRedraw,

    /// No redraws required because the pager display hasen't started
    NoDraw,
}

pub struct FormatOpts<'a> {
    /// Contains the incoming text data
    pub text: &'a str,
    /// This is Some when the last line inside minus's present data is unterminated. It contains the last
    /// line to be attached to the the incoming text
    pub attachment: Option<String>,
    /// Status of line numbers
    pub line_numbers: LineNumbers,
    /// This is equal to the number of lines in [`PagerState::lines`](crate::state::PagerState::lines). This basically tells what line
    /// number the upcoming line will hold.
    pub lines_count: usize,
    /// This is equal to the number of lines in [`PagerState::formatted_lines`](crate::state::PagerState::lines). This is used to
    /// calculate the search index of the rows of the line.
    pub formatted_lines_count: usize,
    /// Actual number of columns available for displaying
    pub cols: usize,
    /// Number of lines that are previously unterminated. It is only relevant when there is `attachment` text otherwise
    /// it should be 0.
    pub prev_unterminated: usize,
    /// Search term if a search is active
    #[cfg(feature = "search")]
    pub search_term: &'a Option<regex::Regex>,

    /// Value of [PagerState::line_wrapping]
    pub line_wrapping: bool,
}

/// Contains the formatted rows along with some basic information about the text formatted
///
/// The basic information includes things like the number of lines formatted or the length of
/// longest line encountered. These are tracked as each line is being formatted hence we refer to
/// them as **tracking variables**.
#[derive(Debug)]
pub struct FormatResult {
    /// Formatted incoming lines
    pub text: Vec<String>,
    //
    // **Tracking variables**
    //
    /// Number of lines that have been formatted from `text`.
    pub lines_formatted: usize,
    /// Number of rows that are unterminated
    pub num_unterminated: usize,
    /// If search is active, this contains the indices where search matches in the incoming text have been found
    #[cfg(feature = "search")]
    pub append_search_idx: BTreeSet<usize>,
    /// Map of where first row of each line is placed inside in
    /// [`PagerState::formatted_lines`](crate::state::PagerState::formatted_lines)
    pub lines_to_row_map: LinesRowMap,
    /// The length of longest line encountered in the formatted text block
    pub max_line_length: usize,
}

/// Makes the text that will be displayed.
#[allow(clippy::too_many_lines)]
pub fn format_text_block(mut opts: FormatOpts<'_>) -> FormatResult {
    // Formatting a text block not only requires us to format each line according to the terminal
    // configuration and the main applications's preference but also gather some basic information
    // about the text that we formatted. The basic information that we gather is supplied along
    // with the formatted lines in the FormatResult's tracking variables.
    //
    // This is a high level overview of how the text formatting works.
    //
    // For a text block, we hae a couple of things to care about:-
    // * Each line is formatted using the using the `formatted_line()` function.
    //   After a line has been formatted using the `formatted_line()` function, calling `.len()` on
    //   the returned vector will give the number of rows that it would span on the terminal.
    //   For less confusion, we call this *row span of that line*.
    // * The first line can have an attachment, in the sense that it can be part of the last line of the
    //   already present text. In that case the FrmatResult::attachment will hold a `Some(...)`
    //   value. `clean_append` keeps track of this: it will be false if an attachment is available.
    // * Formatting of the lines between the first line and last line ie. *middle lines*, is actually
    //   rather simple: we simply format them
    // * The last is also similar to the middle lines except for one exception:-
    //
    //      If it isn't terminated by a \n then we need to find how many rows it
    //      will span in the terminal and set it to the `unterminated` count.
    //
    //   More on this is described in the unterminated section.
    //
    // * We also have more things to take care like `append_search_idx` but most of these
    //   either documented in their respective section or self-understanable so not discussed here.
    //
    // Now the good stuff...
    // * First, if there's an attachment, we merge it with the actual text to be formatted
    //   and tweak certain parameters (see below)
    // * Then  we split the entire text block into two parts: rest_lines and last_line.
    // * Next we format the rest_lines, and all update the tracking variables.
    // * Next we format the last line and keep it separate to calculate unterminated.
    // * If there's exactly one line to format, it will automatically behave as last_line and there
    //   will be no rest_lines.
    // * After all the formatting is done, we return the format results.

    // Compute the text to be format and set clean_append
    let to_format;
    if let Some(ref attached_text) = opts.attachment {
        // Tweak certain parameters if we are joining the last line of already present text with the first line of
        // incoming text.
        //
        // First reduce line count by 1 if, because the first line of the incoming text should have the same line
        // number as the last line. Hence all subsequent lines must get a line number less than expected.
        //
        // Next subtract the number of rows that the last line occupied from formatted_lines_count since it is
        // also getting reformatted. This can be easily accomplished by taking help of [`PagerState::unterminated`]
        // which we get in opts.prev_unterminated.
        opts.lines_count = opts.lines_count.saturating_sub(1);
        opts.formatted_lines_count = opts
            .formatted_lines_count
            .saturating_sub(opts.prev_unterminated);
        let mut s = String::with_capacity(opts.text.len() + attached_text.len());
        s.push_str(attached_text);
        s.push_str(opts.text);

        to_format = s;
    } else {
        to_format = opts.text.to_string();
    }

    let lines = to_format
        .lines()
        .enumerate()
        .collect::<Vec<(usize, &str)>>();

    let to_format_size = lines.len();

    let mut fr = FormatResult {
        text: Vec::with_capacity(0),
        lines_formatted: to_format_size,
        num_unterminated: opts.prev_unterminated,
        #[cfg(feature = "search")]
        append_search_idx: BTreeSet::new(),
        lines_to_row_map: LinesRowMap::new(),
        max_line_length: 0,
    };

    let line_number_digits = minus_core::utils::digits(opts.lines_count + to_format_size);

    // Return if we have nothing to format
    if lines.is_empty() {
        return fr;
    }

    // A container for the individual rows to be stored.
    let mut fmt_lines = Vec::with_capacity(256);

    // Number of rows that have been formatted so far
    // Whenever a line is formatted, this will be incremented to te number of rows that the formatted line has occupied
    let mut formatted_row_count = opts.formatted_lines_count;

    let resy_lines = lines
        .iter()
        .take(lines.len().saturating_sub(1))
        .flat_map(|(idx, line)| {
            let fmt_line = formatted_line(
                line,
                line_number_digits,
                opts.lines_count + idx,
                opts.line_numbers,
                opts.cols,
                opts.line_wrapping,
                #[cfg(feature = "search")]
                formatted_row_count,
                #[cfg(feature = "search")]
                &mut fr.append_search_idx,
                #[cfg(feature = "search")]
                opts.search_term,
            );
            fr.lines_to_row_map.insert(formatted_row_count, true);
            formatted_row_count += fmt_line.len();
            if lines.len() > fr.max_line_length {
                fr.max_line_length = line.len();
            }

            fmt_line
        });
    fmt_lines.extend(resy_lines);

    let mut last_line = formatted_line(
        lines.last().unwrap().1,
        line_number_digits,
        opts.lines_count + to_format_size - 1,
        opts.line_numbers,
        opts.cols,
        opts.line_wrapping,
        #[cfg(feature = "search")]
        formatted_row_count,
        #[cfg(feature = "search")]
        &mut fr.append_search_idx,
        #[cfg(feature = "search")]
        opts.search_term,
    );
    fr.lines_to_row_map.insert(formatted_row_count, true);
    if lines.last().unwrap().1.len() > fr.max_line_length {
        fr.max_line_length = lines.last().unwrap().1.len();
    }

    #[cfg(feature = "search")]
    {
        // NOTE: VERY IMPORTANT BLOCK TO GET PROPER SEARCH INDEX
        // Here is the current scenario: suppose you have text block like this (markers are present to denote where a
        // new line begins).
        //
        // * This is line one row one
        //   This is line one row two
        //   This is line one row three
        // * This is line two row one
        //   This is line two row two
        //   This is line two row three
        //   This is line two row four
        //
        // and suppose a match is found at line 1 row 2 and line 2 row 4. So the index generated will be [1, 6].
        // Let's say this text block is going to be appended to [PagerState::formatted_lines] from index 23.
        // Now if directly append this generated index to [`PagerState::search_idx`], it will probably be wrong
        // as these numbers are *relative to current text block*. The actual search index should have been 24, 30.
        //
        // To fix this we basically add the number of items in [`PagerState::formatted_lines`].
        fr.append_search_idx = fr
            .append_search_idx
            .iter()
            .map(|i| opts.formatted_lines_count + i)
            .collect();
    }

    // Calculate number of rows which are part of last line and are left unterminated  due to absence of \n
    fr.num_unterminated = if opts.text.ends_with('\n') {
        // If the last line ends with \n, then the line is complete so nothing is left as unterminated
        0
    } else {
        last_line.len()
    };

    fmt_lines.append(&mut last_line);

    fr.text = fmt_lines;

    fr
}

/// Formats the given `line`
///
/// - `line`: The line to format
/// - `line_numbers`: tells whether to format the line with line numbers.
/// - `len_line_number`: is the number of digits that number of lines in [`PagerState::lines`] occupy.
///     For example, this will be 2 if number of lines in [`PagerState::lines`] is 50 and 3 if
///     number of lines in [`PagerState::lines`] is 500. This is used for calculating the padding
///     of each displayed line.
/// - `idx`: is the position index where the line is placed in [`PagerState::lines`].
/// - `formatted_idx`: is the position index where the line will be placed in the resulting
///    [`PagerState::formatted_lines`](crate::state::PagerState::formatted_lines)
/// - `cols`: Number of columns in the terminal
/// - `search_term`: Contains the regex if a search is active
///
/// [`PagerState::lines`]: crate::state::PagerState::lines
#[allow(clippy::too_many_arguments)]
#[allow(clippy::uninlined_format_args)]
pub fn formatted_line<'a>(
    line: &'a str,
    len_line_number: usize,
    idx: usize,
    line_numbers: LineNumbers,
    cols: usize,
    line_wrapping: bool,
    #[cfg(feature = "search")] formatted_idx: usize,
    #[cfg(feature = "search")] search_idx: &mut BTreeSet<usize>,
    #[cfg(feature = "search")] search_term: &Option<regex::Regex>,
) -> Vec<String> {
    assert!(
        !line.contains('\n'),
        "Newlines found in appending line {:?}",
        line
    );
    let line_numbers = matches!(line_numbers, LineNumbers::Enabled | LineNumbers::AlwaysOn);

    // NOTE: Only relevant when line numbers are active
    // Padding is the space that the actual line text will be shifted to accommodate for
    // line numbers. This is equal to:-
    // LineNumbers::EXTRA_PADDING + len_line_number + 1 (for '.') + 1 (for 1 space)
    //
    // We reduce this from the number of available columns as this space cannot be used for
    // actual line display when wrapping the lines
    let padding = len_line_number + LineNumbers::EXTRA_PADDING + 1;

    let cols_avail = if line_numbers {
        cols.saturating_sub(padding + 2)
    } else {
        cols
    };

    // Wrap the line and return an iterator over all the rows
    let mut enumerated_rows = if line_wrapping {
        textwrap::wrap(line, cols_avail)
    } else {
        vec![Cow::from(line)]
    }
    .into_iter()
    .enumerate();

    // highlight the lines with matching search terms
    // If a match is found, add this line's index to PagerState::search_idx
    #[cfg_attr(not(feature = "search"), allow(unused_mut))]
    #[cfg_attr(not(feature = "search"), allow(unused_variables))]
    let mut handle_search = |row: &mut Cow<'a, str>, wrap_idx: usize| {
        #[cfg(feature = "search")]
        if let Some(st) = search_term.as_ref() {
            let (highlighted_row, is_match) = search::highlight_line_matches(row, st, false);
            if is_match {
                *row.to_mut() = highlighted_row;
                search_idx.insert(formatted_idx + wrap_idx);
            }
        }
    };

    if line_numbers {
        let mut formatted_rows = Vec::with_capacity(256);

        // Formatter for only when line numbers are active
        // * If minus is run under test, ascii codes for making the numbers bol is not inserted because they add
        // extra difficulty while writing tests
        // * Line number is added only to the first row of a line. This makes a better UI overall
        let formatter = |row: Cow<'_, str>, is_first_row: bool, idx: usize| {
            format!(
                "{bold}{number: >len$}{reset} {row}",
                bold = if cfg!(not(test)) && is_first_row {
                    crossterm::style::Attribute::Bold.to_string()
                } else {
                    String::new()
                },
                number = if is_first_row {
                    (idx + 1).to_string() + "."
                } else {
                    String::new()
                },
                len = padding,
                reset = if cfg!(not(test)) && is_first_row {
                    crossterm::style::Attribute::Reset.to_string()
                } else {
                    String::new()
                },
                row = row
            )
        };

        // First format the first row separate from other rows, then the subsequent rows and finally join them
        // This is because only the first row contains the line number and not the subsequent rows
        let first_row = {
            #[cfg_attr(not(feature = "search"), allow(unused_mut))]
            let mut row = enumerated_rows.next().unwrap().1;
            handle_search(&mut row, 0);
            formatter(row, true, idx)
        };
        formatted_rows.push(first_row);

        #[cfg_attr(not(feature = "search"), allow(unused_mut))]
        #[cfg_attr(not(feature = "search"), allow(unused_variables))]
        let rows_left = enumerated_rows.map(|(wrap_idx, mut row)| {
            handle_search(&mut row, wrap_idx);
            formatter(row, false, 0)
        });
        formatted_rows.extend(rows_left);

        formatted_rows
    } else {
        // If line numbers aren't active, simply return the rows with search matches highlighted if search is active
        #[cfg_attr(not(feature = "search"), allow(unused_variables))]
        enumerated_rows
            .map(|(wrap_idx, mut row)| {
                handle_search(&mut row, wrap_idx);
                row.to_string()
            })
            .collect::<Vec<String>>()
    }
}

pub fn make_format_lines(
    text: &String,
    line_numbers: LineNumbers,
    cols: usize,
    line_wrapping: bool,
    #[cfg(feature = "search")] search_term: &Option<regex::Regex>,
) -> FormatResult {
    let format_opts = FormatOpts {
        text,
        attachment: None,
        line_numbers,
        formatted_lines_count: 0,
        lines_count: 0,
        prev_unterminated: 0,
        cols,
        #[cfg(feature = "search")]
        search_term,
        line_wrapping,
    };

    format_text_block(format_opts)
}

#[cfg(test)]
mod unterminated {
    use super::{format_text_block, FormatOpts};

    const fn get_append_opts_template(text: &str) -> FormatOpts {
        FormatOpts {
            text,
            attachment: None,
            #[cfg(feature = "search")]
            search_term: &None,
            lines_count: 0,
            formatted_lines_count: 0,
            cols: 80,
            line_numbers: crate::LineNumbers::Disabled,
            prev_unterminated: 0,
            line_wrapping: true,
        }
    }

    #[test]
    fn test_single_no_endline() {
        let append_style = format_text_block(get_append_opts_template("This is a line"));
        assert_eq!(1, append_style.num_unterminated);
    }

    #[test]
    fn test_single_endline() {
        let append_style = format_text_block(get_append_opts_template("This is a line\n"));
        assert_eq!(0, append_style.num_unterminated);
    }

    #[test]
    fn test_single_multi_newline() {
        let append_style = format_text_block(get_append_opts_template(
            "This is a line\nThis is another line\nThis is third line",
        ));
        assert_eq!(1, append_style.num_unterminated);
    }

    #[test]
    fn test_single_multi_endline() {
        let append_style = format_text_block(get_append_opts_template(
            "This is a line\nThis is another line\n",
        ));
        assert_eq!(0, append_style.num_unterminated);
    }

    #[test]
    fn test_single_line_wrapping() {
        let mut fs = get_append_opts_template("This is a quite lengthy line");
        fs.cols = 20;
        let append_style = format_text_block(fs);
        assert_eq!(2, append_style.num_unterminated);
    }

    #[test]
    fn test_single_mid_newline_wrapping() {
        let mut fs = get_append_opts_template(
            "This is a quite lengthy line\nIt has three lines\nThis is
third line",
        );
        fs.cols = 20;
        let append_style = format_text_block(fs);
        assert_eq!(1, append_style.num_unterminated);
    }

    #[test]
    fn test_single_endline_wrapping() {
        let mut fs = get_append_opts_template(
            "This is a quite lengthy line\nIt has three lines\nThis is
third line\n",
        );
        fs.cols = 20;
        let append_style = format_text_block(fs);
        assert_eq!(0, append_style.num_unterminated);
    }

    #[test]
    fn test_multi_no_endline() {
        let append_style = format_text_block(get_append_opts_template("This is a line. "));
        assert_eq!(1, append_style.num_unterminated);

        let mut fs = get_append_opts_template("This is another line");
        fs.prev_unterminated = append_style.num_unterminated;
        fs.attachment = Some("This is a line. ".to_string());

        let append_style = format_text_block(fs);
        assert_eq!(1, append_style.num_unterminated);
    }

    #[test]
    fn test_multi_endline() {
        let append_style = format_text_block(get_append_opts_template("This is a line. "));
        assert_eq!(1, append_style.num_unterminated);

        let mut fs = get_append_opts_template("This is another line\n");
        fs.prev_unterminated = append_style.num_unterminated;
        fs.attachment = Some("This is a line. ".to_string());

        let append_style = format_text_block(fs);
        assert_eq!(0, append_style.num_unterminated);
    }

    #[test]
    fn test_multi_multiple_newline() {
        let append_style = format_text_block(get_append_opts_template("This is a line\n"));
        assert_eq!(0, append_style.num_unterminated);

        let mut fs = get_append_opts_template("This is another line\n");
        fs.lines_count = 1;
        fs.formatted_lines_count = 1;
        fs.attachment = None;

        let append_style = format_text_block(fs);
        assert_eq!(0, append_style.num_unterminated);
    }

    #[test]
    fn test_multi_wrapping() {
        let mut fs = get_append_opts_template("This is a line. This is second line. ");
        fs.cols = 20;
        let append_style = format_text_block(fs);
        assert_eq!(2, append_style.num_unterminated);

        let mut fs = get_append_opts_template("This is another line\n");
        fs.cols = 20;
        fs.prev_unterminated = append_style.num_unterminated;
        fs.attachment = Some("This is a line. This is second line".to_string());

        let append_style = format_text_block(fs);
        assert_eq!(0, append_style.num_unterminated);
    }

    #[test]
    fn test_multi_wrapping_continued() {
        let mut fs = get_append_opts_template("This is a line. This is second line. ");
        fs.cols = 20;
        let append_style = format_text_block(fs);
        assert_eq!(2, append_style.num_unterminated);

        let mut fs = get_append_opts_template("This is third line");
        fs.cols = 20;
        fs.prev_unterminated = append_style.num_unterminated;
        fs.attachment = Some("This is a line. This is second line. ".to_string());

        let append_style = format_text_block(fs);
        assert_eq!(3, append_style.num_unterminated);
    }

    #[test]
    fn test_multi_wrapping_last_continued() {
        let mut fs = get_append_opts_template("This is a line.\nThis is second line. ");
        fs.cols = 20;
        let append_style = format_text_block(fs);
        assert_eq!(1, append_style.num_unterminated);

        let mut fs = get_append_opts_template("This is third line.");
        fs.cols = 20;
        fs.prev_unterminated = append_style.num_unterminated;
        fs.attachment = Some("This is second line. ".to_string());
        fs.lines_count = 1;
        fs.formatted_lines_count = 2;

        let append_style = format_text_block(fs);

        assert_eq!(2, append_style.num_unterminated);
    }

    #[test]
    fn test_multi_wrapping_additive() {
        let mut fs = get_append_opts_template("This is a line. ");
        fs.cols = 20;
        let append_style = format_text_block(fs);
        assert_eq!(1, append_style.num_unterminated);

        let mut fs = get_append_opts_template("This is second line. ");
        fs.cols = 20;
        fs.prev_unterminated = append_style.num_unterminated;
        fs.attachment = Some("This is a line. ".to_string());

        let append_style = format_text_block(fs);
        assert_eq!(2, append_style.num_unterminated);

        let mut fs = get_append_opts_template("This is third line");
        fs.cols = 20;
        fs.prev_unterminated = append_style.num_unterminated;
        fs.attachment = Some("This is a line. This is second line. ".to_string());
        let append_style = format_text_block(fs);

        assert_eq!(3, append_style.num_unterminated);
    }
}
