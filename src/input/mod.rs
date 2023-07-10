//! Working with user input
//!
//! This module provides various items for wroking with user input from the terminal.
//!
//! minus already has a sensible set of default key/mouse bindings so most people do not need to care about this module.
//! But if you want to add or remove certain key bindings then you need to rely on this module..
//!
//! There are two ways to define inputs in minus
//!
//! # Newer (Recommended) Method
//! This method uses a much improved and ergonomic API for defining the input events. It allows you to add/delete/update
//! inputs without needing to copy the entire default template into the main application's codebase.
//! You also don't need to specifically bring in [`crossterm`] as a dependency for working with this.
//!
//! ## Example:
//! ```
//! use minus::{input::{InputEvent, HashedEventRegister}, Pager};
//!
//! let pager = Pager::new();
//! let mut input_register = HashedEventRegister::default();
//!
//! input_register.add_key_events(&["down"], |_, ps| {
//!     InputEvent::UpdateUpperMark(ps.upper_mark.saturating_sub(1))
//! });
//!
//! input_register.add_key_events(&["q", "c-c"], |_, _| InputEvent::Exit);
//!
//! pager.set_input_classifier(Box::new(input_register));
//! ```
//!
//! # Legacy method
//! This method relies heavily on the [`InputClassifier`] trait and the end-applications needs to bring in the underlying
//! [`crossterm`] crate to define the inputs.
//! Also there is no such option to add/remove/update a set of events. You need to manually copy the
//! [default definitions](DefaultInputClassifier) and make the required modifications youself in this method.
//!
//! ## Example
//! ```
//! use minus::{input::{InputEvent, InputClassifier}, Pager, PagerState};
//! use crossterm::event::{Event, KeyEvent, KeyCode, KeyModifiers};
//!
//! struct CustomInputClassifier;
//! impl InputClassifier for CustomInputClassifier {
//!     fn classify_input(
//!         &self,
//!         ev: Event,
//!         ps: &PagerState
//!     ) -> Option<InputEvent> {
//!             match ev {
//!                 Event::Key(KeyEvent {
//!                     code: KeyCode::Up,
//!                     modifiers: KeyModifiers::NONE,
//!                 })
//!                 | Event::Key(KeyEvent {
//!                     code: KeyCode::Char('j'),
//!                     modifiers: KeyModifiers::NONE,
//!                 }) => Some(InputEvent::UpdateUpperMark
//!                       (ps.upper_mark.saturating_sub(1))),
//!                 _ => None
//!         }
//!     }
//! }
//!
//! let mut pager = Pager::new();
//! pager.set_input_classifier(
//!                 Box::new(CustomInputClassifier)
//!             );
//! ```
//!
//! At the heart of this module is the [`InputEvent`] enum and [`InputClassifier`] trait.
//! The [`InputEvent`] enum defies the various events which minus can properly respond to

pub(crate) mod definitions;
pub(crate) mod event_wrapper;
pub use crossterm::event as crossterm_event;

#[cfg(feature = "search")]
use crate::minus_core::search::SearchMode;
use crate::{LineNumbers, PagerState};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
pub use event_wrapper::HashedEventRegister;

/// Events handled by the `minus` pager.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions)]
pub enum InputEvent {
    /// `Ctrl+C` or `Q`, exits the application.
    Exit,
    /// The terminal was resized. Contains the new number of rows.
    UpdateTermArea(usize, usize),
    /// Sent by movement keys like `Up` `Down`, `PageUp`, 'PageDown', 'g', `G` etc. Contains the new value for the upper mark.
    UpdateUpperMark(usize),
    /// `Ctrl+L`, inverts the line number display. Contains the new value.
    UpdateLineNumber(LineNumbers),
    /// A number key has been pressed. This inner value is stored as a `char`.
    /// The input loop will append this number to its `count` string variable
    Number(char),
    /// Restore the original prompt
    RestorePrompt,
    Ignore,
    /// `/`, Searching for certain pattern of text
    #[cfg(feature = "search")]
    Search(SearchMode),
    /// Get to the next match in forward mode
    #[cfg(feature = "search")]
    NextMatch,
    /// Get to the previous match in forward mode
    #[cfg(feature = "search")]
    PrevMatch,
    /// Move to the next nth match in the given direction
    #[cfg(feature = "search")]
    MoveToNextMatch(usize),
    /// Move to the previous nth match in the given direction
    #[cfg(feature = "search")]
    MoveToPrevMatch(usize),
}

/// Classifies the input and returns the appropriate [`InputEvent`]
///
/// If you are using the newer method for input definition, you don't need to take care of this.
///
/// If you are using the old method, see the sources of [`DefaultInputClassifier`] on how to inplement this trait.
#[allow(clippy::module_name_repetitions)]
pub trait InputClassifier {
    fn classify_input(&self, ev: Event, ps: &PagerState) -> Option<InputEvent>;
}

/// Insert the default set of actions into the [`HashedEventRegister`]
pub fn generate_default_bindings<S>(map: &mut HashedEventRegister<S>)
where
    S: std::hash::BuildHasher,
{
    map.add_key_events(&["q", "c-c"], |_, _| InputEvent::Exit);

    map.add_key_events(&["up", "k"], |_, ps| {
        let position = ps.prefix_num.parse::<usize>().unwrap_or(1);
        InputEvent::UpdateUpperMark(ps.upper_mark.saturating_sub(position))
    });
    map.add_key_events(&["down", "j"], |_, ps| {
        let position = ps.prefix_num.parse::<usize>().unwrap_or(1);
        InputEvent::UpdateUpperMark(ps.upper_mark.saturating_add(position))
    });
    map.add_key_events(&["enter"], |_, ps| {
        if ps.message.is_some() {
            InputEvent::RestorePrompt
        } else {
            let position = ps.prefix_num.parse::<usize>().unwrap_or(1);
            InputEvent::UpdateUpperMark(ps.upper_mark.saturating_add(position))
        }
    });
    map.add_key_events(&["u", "c-u"], |_, ps| {
        let half_screen = ps.rows / 2;
        InputEvent::UpdateUpperMark(ps.upper_mark.saturating_sub(half_screen))
    });
    map.add_key_events(&["d", "c-d"], |_, ps| {
        let half_screen = ps.rows / 2;
        InputEvent::UpdateUpperMark(ps.upper_mark.saturating_add(half_screen))
    });
    map.add_key_events(&["g"], |_, _| InputEvent::UpdateUpperMark(0));

    map.add_key_events(&["s-g", "s-G", "G"], |_, ps| {
        let mut position = ps
            .prefix_num
            .parse::<usize>()
            .unwrap_or(usize::MAX)
            // Reduce 1 here, because line numbering starts from 1
            // while upper_mark starts from 0
            .saturating_sub(1);
        if position == 0 {
            position = usize::MAX;
        }
        InputEvent::UpdateUpperMark(position)
    });
    map.add_key_events(&["pageup"], |_, ps| {
        InputEvent::UpdateUpperMark(ps.upper_mark.saturating_sub(ps.rows - 1))
    });
    map.add_key_events(&["pagedown", "space"], |_, ps| {
        InputEvent::UpdateUpperMark(ps.upper_mark.saturating_add(ps.rows - 1))
    });
    map.add_key_events(&["c-l"], |_, ps| {
        InputEvent::UpdateLineNumber(!ps.line_numbers)
    });
    #[cfg(feature = "search")]
    {
        map.add_key_events(&["/"], |_, _| InputEvent::Search(SearchMode::Forward));
        map.add_key_events(&["?"], |_, _| InputEvent::Search(SearchMode::Reverse));
        map.add_key_events(&["n"], |_, ps| {
            if ps.search_mode == SearchMode::Forward {
                InputEvent::MoveToNextMatch(1)
            } else if ps.search_mode == SearchMode::Reverse {
                InputEvent::MoveToPrevMatch(1)
            } else {
                InputEvent::Ignore
            }
        });
        map.add_key_events(&["p"], |_, ps| {
            if ps.search_mode == SearchMode::Forward {
                InputEvent::MoveToPrevMatch(1)
            } else if ps.search_mode == SearchMode::Reverse {
                InputEvent::MoveToNextMatch(1)
            } else {
                InputEvent::Ignore
            }
        });
    }

    map.add_mouse_events(&["scroll:up"], |_, ps| {
        InputEvent::UpdateUpperMark(ps.upper_mark.saturating_sub(5))
    });
    map.add_mouse_events(&["scroll:down"], |_, ps| {
        InputEvent::UpdateUpperMark(ps.upper_mark.saturating_add(5))
    });

    map.add_resize_event(|ev, _| {
        let Event::Resize(cols, rows) = ev else {
            unreachable!();
        };
        InputEvent::UpdateTermArea(cols as usize, rows as usize)
    });

    map.insert_wild_event_matcher(|ev, _| {
        if let Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
        }) = ev
        {
            if c.is_ascii_digit() {
                InputEvent::Number(c)
            } else {
                InputEvent::Ignore
            }
        } else {
            InputEvent::Ignore
        }
    });
}

/// The default set of input definitions
///
/// **This is kept only for legacy purposes and may not be well updated with all the latest changes**
pub struct DefaultInputClassifier;

impl InputClassifier for DefaultInputClassifier {
    #[allow(clippy::too_many_lines)]
    fn classify_input(&self, ev: Event, ps: &PagerState) -> Option<InputEvent> {
        #[allow(clippy::unnested_or_patterns)]
        match ev {
            // Scroll up by one.
            Event::Key(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
            }) if code == KeyCode::Up || code == KeyCode::Char('k') => {
                let position = ps.prefix_num.parse::<usize>().unwrap_or(1);
                Some(InputEvent::UpdateUpperMark(
                    ps.upper_mark.saturating_sub(position),
                ))
            }

            // Scroll down by one.
            Event::Key(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
            }) if code == KeyCode::Down || code == KeyCode::Char('j') => {
                let position = ps.prefix_num.parse::<usize>().unwrap_or(1);
                Some(InputEvent::UpdateUpperMark(
                    ps.upper_mark.saturating_add(position),
                ))
            }

            // For number keys
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
            }) if c.is_ascii_digit() => Some(InputEvent::Number(c)),

            // Enter key
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            }) => {
                if ps.message.is_some() {
                    Some(InputEvent::RestorePrompt)
                } else {
                    let position = ps.prefix_num.parse::<usize>().unwrap_or(1);
                    Some(InputEvent::UpdateUpperMark(
                        ps.upper_mark.saturating_add(position),
                    ))
                }
            }

            // Scroll up by half screen height.
            Event::Key(KeyEvent {
                code: KeyCode::Char('u'),
                modifiers,
            }) if modifiers == KeyModifiers::CONTROL || modifiers == KeyModifiers::NONE => {
                let half_screen = ps.rows / 2;
                Some(InputEvent::UpdateUpperMark(
                    ps.upper_mark.saturating_sub(half_screen),
                ))
            }
            // Scroll down by half screen height.
            Event::Key(KeyEvent {
                code: KeyCode::Char('d'),
                modifiers,
            }) if modifiers == KeyModifiers::CONTROL || modifiers == KeyModifiers::NONE => {
                let half_screen = ps.rows / 2;
                Some(InputEvent::UpdateUpperMark(
                    ps.upper_mark.saturating_add(half_screen),
                ))
            }

            // Mouse scroll up/down
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollUp,
                ..
            }) => Some(InputEvent::UpdateUpperMark(ps.upper_mark.saturating_sub(5))),
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                ..
            }) => Some(InputEvent::UpdateUpperMark(ps.upper_mark.saturating_add(5))),
            // Go to top.
            Event::Key(KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::NONE,
            }) => Some(InputEvent::UpdateUpperMark(0)),
            // Go to bottom.
            Event::Key(KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::SHIFT,
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('G'),
                modifiers: KeyModifiers::SHIFT,
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('G'),
                modifiers: KeyModifiers::NONE,
            }) => {
                let mut position = ps
                    .prefix_num
                    .parse::<usize>()
                    .unwrap_or(usize::MAX)
                    // Reduce 1 here, because line numbering starts from 1
                    // while upper_mark starts from 0
                    .saturating_sub(1);
                if position == 0 {
                    position = usize::MAX;
                }
                Some(InputEvent::UpdateUpperMark(position))
            }

            // Page Up/Down
            Event::Key(KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
            }) => Some(InputEvent::UpdateUpperMark(
                ps.upper_mark.saturating_sub(ps.rows - 1),
            )),
            Event::Key(KeyEvent {
                code: c,
                modifiers: KeyModifiers::NONE,
            }) if c == KeyCode::PageDown || c == KeyCode::Char(' ') => Some(
                InputEvent::UpdateUpperMark(ps.upper_mark.saturating_add(ps.rows - 1)),
            ),

            // Resize event from the terminal.
            Event::Resize(cols, rows) => {
                Some(InputEvent::UpdateTermArea(cols as usize, rows as usize))
            }
            // Switch line number display.
            Event::Key(KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
            }) => Some(InputEvent::UpdateLineNumber(!ps.line_numbers)),
            // Quit.
            Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
            }) => Some(InputEvent::Exit),
            #[cfg(feature = "search")]
            Event::Key(KeyEvent {
                code: KeyCode::Char('/'),
                modifiers: KeyModifiers::NONE,
            }) => Some(InputEvent::Search(SearchMode::Forward)),
            #[cfg(feature = "search")]
            Event::Key(KeyEvent {
                code: KeyCode::Char('?'),
                modifiers: KeyModifiers::NONE,
            }) => Some(InputEvent::Search(SearchMode::Reverse)),
            #[cfg(feature = "search")]
            Event::Key(KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::NONE,
            }) => {
                let position = ps.prefix_num.parse::<usize>().unwrap_or(1);
                if ps.search_mode == SearchMode::Reverse {
                    Some(InputEvent::MoveToPrevMatch(position))
                } else {
                    Some(InputEvent::MoveToNextMatch(position))
                }
            }
            #[cfg(feature = "search")]
            Event::Key(KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::NONE,
            }) => {
                let position = ps.prefix_num.parse::<usize>().unwrap_or(1);
                if ps.search_mode == SearchMode::Reverse {
                    Some(InputEvent::MoveToNextMatch(position))
                } else {
                    Some(InputEvent::MoveToPrevMatch(position))
                }
            }
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests;
