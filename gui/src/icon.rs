// Generated automatically by iced_fontello at build time.
// Do not edit manually. Source: ../fonts/icons.toml
// f12692990a5142f3aa5a84c759b0e41dcdf4fa534167effcec6e72d4395b9a94
use iced::Font;
use iced::widget::{Text, text};

pub const FONT: &[u8] = include_bytes!("../fonts/icons.ttf");

pub fn caret_down<'a>() -> Text<'a> {
    icon("\u{F107}")
}

pub fn caret_up<'a>() -> Text<'a> {
    icon("\u{F106}")
}

pub fn cog<'a>() -> Text<'a> {
    icon("\u{2699}")
}

pub fn copy<'a>() -> Text<'a> {
    icon("\u{F0C5}")
}

pub fn edit<'a>() -> Text<'a> {
    icon("\u{270E}")
}

pub fn file<'a>() -> Text<'a> {
    icon("\u{1F4C4}")
}

pub fn folder<'a>() -> Text<'a> {
    icon("\u{F114}")
}

pub fn minus<'a>() -> Text<'a> {
    icon("\u{2D}")
}

pub fn plus<'a>() -> Text<'a> {
    icon("\u{2B}")
}

pub fn save<'a>() -> Text<'a> {
    icon("\u{1F4BE}")
}

pub fn trash<'a>() -> Text<'a> {
    icon("\u{E10A}")
}

fn icon(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("icons"))
}
