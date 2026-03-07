// Generated automatically by iced_fontello at build time.
// Do not edit manually. Source: ../fonts/icons.toml
// 9c864d13279985db8adb885aeeb385f802e5a3fc47845c9707ad1b70857d0727
use iced::Font;
use iced::widget::{Text, text};

pub const FONT: &[u8] = include_bytes!("../fonts/icons.ttf");

pub fn edit<'a>() -> Text<'a> {
    icon("\u{270E}")
}

pub fn file<'a>() -> Text<'a> {
    icon("\u{1F4C4}")
}

pub fn minus<'a>() -> Text<'a> {
    icon("\u{2D}")
}

pub fn opendir<'a>() -> Text<'a> {
    icon("\u{F115}")
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
