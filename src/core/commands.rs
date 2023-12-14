//! Provides the [`Event`] enum and all its related implementations
use std::fmt::Debug;

use crate::{
    input::{InputClassifier, InputEvent},
    ExitStrategy, LineNumbers,
};

#[cfg(feature = "search")]
use crate::search::SearchOpts;

/// Different events that can be encountered while the pager is running
#[non_exhaustive]
pub enum Command {
    AppendData(String),
    SetData(String),
    UserInput(InputEvent),
    SetPrompt(String),
    SendMessage(String),
    SetLineNumbers(LineNumbers),
    SetExitStrategy(ExitStrategy),
    SetInputClassifier(Box<dyn InputClassifier + Send + Sync + 'static>),
    AddExitCallback(Box<dyn FnMut() + Send + Sync + 'static>),
    ShowPrompt(bool),
    FollowOutput(bool),
    #[cfg(feature = "static_output")]
    SetRunNoOverflow(bool),
    #[cfg(feature = "search")]
    IncrementalSearchCondition(Box<dyn Fn(&SearchOpts) -> bool + Send + Sync + 'static>),
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::SetData(d1), Self::SetData(d2))
            | (Self::AppendData(d1), Self::AppendData(d2))
            | (Self::SetPrompt(d1), Self::SetPrompt(d2))
            | (Self::SendMessage(d1), Self::SendMessage(d2)) => d1 == d2,
            (Self::SetLineNumbers(d1), Self::SetLineNumbers(d2)) => d1 == d2,
            (Self::ShowPrompt(d1), Self::ShowPrompt(d2)) => d1 == d2,
            (Self::SetExitStrategy(d1), Self::SetExitStrategy(d2)) => d1 == d2,
            #[cfg(feature = "static_output")]
            (Self::SetRunNoOverflow(d1), Self::SetRunNoOverflow(d2)) => d1 == d2,
            (Self::SetInputClassifier(_), Self::SetInputClassifier(_))
            | (Self::AddExitCallback(_), Self::AddExitCallback(_)) => true,
            #[cfg(feature = "search")]
            (Self::IncrementalSearchCondition(_), Self::IncrementalSearchCondition(_)) => true,
            _ => false,
        }
    }
}

impl Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetData(text) => write!(f, "SetData({text:?})"),
            Self::AppendData(text) => write!(f, "AppendData({text:?})"),
            Self::SetPrompt(text) => write!(f, "SetPrompt({text:?})"),
            Self::SendMessage(text) => write!(f, "SendMessage({text:?})"),
            Self::SetLineNumbers(ln) => write!(f, "SetLineNumbers({ln:?})"),
            Self::SetExitStrategy(es) => write!(f, "SetExitStrategy({es:?})"),
            Self::SetInputClassifier(_) => write!(f, "SetInputClassifier"),
            Self::ShowPrompt(show) => write!(f, "ShowPrompt({show:?})"),
            #[cfg(feature = "search")]
            Self::IncrementalSearchCondition(_) => write!(f, "IncrementalSearchCondition"),
            Self::AddExitCallback(_) => write!(f, "AddExitCallback"),
            #[cfg(feature = "static_output")]
            Self::SetRunNoOverflow(val) => write!(f, "SetRunNoOverflow({val:?})"),
            Self::UserInput(input) => write!(f, "UserInput({input:?})"),
            Self::FollowOutput(follow_output) => write!(f, "FollowOutput({follow_output:?})"),
        }
    }
}

impl Command {
    #[allow(dead_code)]
    pub(crate) const fn is_exit_event(&self) -> bool {
        matches!(self, Self::UserInput(InputEvent::Exit))
    }

    #[allow(dead_code)]
    pub(crate) const fn is_movement(&self) -> bool {
        matches!(self, Self::UserInput(InputEvent::UpdateUpperMark(_)))
    }

    #[cfg(feature = "dynamic_output")]
    pub(crate) const fn required_immediate_screen_update(&self) -> bool {
        matches!(
            self,
            Self::SetData(_) | Self::SetPrompt(_) | Self::SendMessage(_) | Self::ShowPrompt(_)
        )
    }
}
