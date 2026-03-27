// Generated automatically by iced_fontello at build time.
// Do not edit manually. Source: ../fonts/icons.toml
// 87ac20c4c8eaf3eb7d06e9309c4caf7032f45a7df2fa391f5d6c506d10bfdbae
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
