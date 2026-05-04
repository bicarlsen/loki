//! App settings.

const THEMES: [&str; 22] = [
    "Light",
    "Dark",
    "Dracula",
    "Nord",
    "Solarized Light",
    "Solarized Dark",
    "Gruvbox Light",
    "Gruvbox Dark",
    "Catppuccin Latte",
    "Catppuccin Frappe",
    "Catppuccin Macchiato",
    "Catppuccin Mocha",
    "Tokyo Night",
    "Tokyo Night Storm",
    "Tokyo Night Light",
    "Kanagawa Wave",
    "Kanagawa Dragon",
    "Kanagawa Lotus",
    "Moonfly",
    "Nightfly",
    "Oxocarbon",
    "Ferra",
];

#[derive(Clone, Debug)]
pub enum Message {
    UpdateTheme(&'static str),
}

pub enum Action {
    None,
    Run(iced::Task<Message>),
}

pub struct AppSettings {
    pub(super) theme: iced::Theme,
}

impl AppSettings {
    pub fn view(&self) -> iced::Element<'_, Message> {
        let pl_theme_selected = theme_to_str(&self.theme);
        let pl_theme = iced::widget::pick_list(THEMES, pl_theme_selected, Message::UpdateTheme);
        let pl_theme = iced::widget::row![iced::widget::text("Theme"), pl_theme];
        iced::widget::column![pl_theme].into()
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::UpdateTheme(theme) => {
                self.theme = if let Some(theme) = str_to_theme(theme) {
                    theme
                } else {
                    iced::Theme::CatppuccinMacchiato
                };

                Action::None
            }
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: iced::Theme::CatppuccinMacchiato,
        }
    }
}

fn theme_to_str(theme: &iced::Theme) -> Option<&'static str> {
    use iced::Theme;

    match theme {
        Theme::Light => Some("Light"),
        Theme::Dark => Some("Dark"),
        Theme::Dracula => Some("Dracula"),
        Theme::Nord => Some("Nord"),
        Theme::SolarizedLight => Some("Solarized Light"),
        Theme::SolarizedDark => Some("Solarized Dark"),
        Theme::GruvboxLight => Some("Gruvbox Light"),
        Theme::GruvboxDark => Some("Gruvbox Dark"),
        Theme::CatppuccinLatte => Some("Catppuccin Latte"),
        Theme::CatppuccinFrappe => Some("Catppuccin Frappe"),
        Theme::CatppuccinMacchiato => Some("Catppuccin Macchiato"),
        Theme::CatppuccinMocha => Some("Catppuccin Mocha"),
        Theme::TokyoNight => Some("Tokyo Night"),
        Theme::TokyoNightStorm => Some("Tokyo Night Storm"),
        Theme::TokyoNightLight => Some("Tokyo Night Light"),
        Theme::KanagawaWave => Some("Kanagawa Wave"),
        Theme::KanagawaDragon => Some("Kanagawa Dragon"),
        Theme::KanagawaLotus => Some("Kanagawa Lotus"),
        Theme::Moonfly => Some("Moonfly"),
        Theme::Nightfly => Some("Nightfly"),
        Theme::Oxocarbon => Some("Oxocarbon"),
        Theme::Ferra => Some("Ferra"),
        Theme::Custom(_) => None,
    }
}

fn str_to_theme(theme: impl AsRef<str>) -> Option<iced::Theme> {
    use iced::Theme;

    match theme.as_ref() {
        "Light" => Some(Theme::Light),
        "Dark" => Some(Theme::Dark),
        "Dracula" => Some(Theme::Dracula),
        "Nord" => Some(Theme::Nord),
        "Solarized Light" => Some(Theme::SolarizedLight),
        "Solarized Dark" => Some(Theme::SolarizedDark),
        "Gruvbox Light" => Some(Theme::GruvboxLight),
        "Gruvbox Dark" => Some(Theme::GruvboxDark),
        "Catppuccin Latte" => Some(Theme::CatppuccinLatte),
        "Catppuccin Frappe" => Some(Theme::CatppuccinFrappe),
        "Catppuccin Macchiato" => Some(Theme::CatppuccinMacchiato),
        "Catppuccin Mocha" => Some(Theme::CatppuccinMocha),
        "Tokyo Night" => Some(Theme::TokyoNight),
        "Tokyo Night Storm" => Some(Theme::TokyoNightStorm),
        "Tokyo Night Light" => Some(Theme::TokyoNightLight),
        "Kanagawa Wave" => Some(Theme::KanagawaWave),
        "Kanagawa Dragon" => Some(Theme::KanagawaDragon),
        "Kanagawa Lotus" => Some(Theme::KanagawaLotus),
        "Moonfly" => Some(Theme::Moonfly),
        "Nightfly" => Some(Theme::Nightfly),
        "Oxocarbon" => Some(Theme::Oxocarbon),
        "Ferra" => Some(Theme::Ferra),
        _ => None,
    }
}
