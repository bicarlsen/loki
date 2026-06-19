use super::plot;

#[derive(Debug, Clone)]
pub enum Message {
    SetMode(plot::mode::Kind),
}

pub enum Action {
    None,
}

#[derive(Default, Debug)]
#[cfg_attr(feature = "project", derive(serde::Serialize, serde::Deserialize))]
pub struct Settings {
    /// Plot mode.
    mode: plot::mode::Kind,
}

impl Settings {
    pub fn new(mode: plot::mode::Kind) -> Self {
        Self { mode }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::SetMode(mode) => {
                self.mode = mode;
                Action::None
            }
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        let title = iced::widget::text("Settings");
        let pl_mode = self.view_mode();

        iced::widget::column![title, pl_mode].into()
    }

    #[inline]
    fn view_mode(&self) -> iced::Element<'_, Message> {
        let pl_mode = iced::widget::pick_list(
            [
                plot::mode::Kind::Scatter,
                plot::mode::Kind::Heatmap,
                plot::mode::Kind::Datacube,
            ],
            Some(self.mode),
            Message::SetMode,
        );

        iced::widget::row![iced::widget::text("Plot mode"), pl_mode].into()
    }
}
