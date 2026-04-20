use super::SharedDataframe;
use iced::widget;
use polars::prelude as pl;

#[derive(Debug, Clone)]
pub enum Message {
    /// Highlight the record at the given index.
    HighlightRecord(usize),
    /// No record should be highlighted.
    ClearHighlight,
}

pub enum Action {
    None,
}

pub struct DataTable {
    df: SharedDataframe,
    highlight: Option<usize>,
}

impl DataTable {
    pub fn new(df: SharedDataframe) -> Self {
        Self {
            df,
            highlight: Default::default(),
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::HighlightRecord(idx) => {
                let _ = self.highlight.insert(idx);
                Action::None
            }
            Message::ClearHighlight => {
                let _ = self.highlight.take();
                Action::None
            }
        }
    }

    pub fn view(
        &self,
        theme: &iced::advanced::graphics::core::Theme,
    ) -> iced::Element<'_, Message> {
        let df = self.df.read().expect("dataframe should be readable");
        // TODO: Headers should be sticky
        // TODO: Columns fit to data instead of header title causing overflow
        let columns = df.schema().iter().map(|(name, _dtype)| {
            widget::table::column(name.as_str(), |idx: usize| {
                let col = df.column(name.as_str()).unwrap();
                let mut text = match col.get(idx).unwrap() {
                    pl::AnyValue::Null => widget::text(""),
                    pl::AnyValue::Boolean(value) => {
                        if value {
                            widget::text("true")
                        } else {
                            widget::text("false")
                        }
                    }
                    pl::AnyValue::String(value) => widget::text(value.to_string()),
                    pl::AnyValue::Float64(value) => widget::text(format!("{value:?}")),
                    pl::AnyValue::UInt8(value) => widget::text(format!("{value:?}")),
                    pl::AnyValue::Int64(value) => widget::text(format!("{value:?}")),
                    pl::AnyValue::Int128(value) => widget::text(format!("{value:?}")),
                    value => todo!("data table display {value:?}"),
                };
                if let Some(highlight) = &self.highlight {
                    if idx == *highlight {
                        text = text.color(theme.palette().success);
                    }
                }
                text
            })
        });

        let table = widget::table::Table::new(columns, 0..df.height());
        widget::scrollable(table)
            .direction(iced::widget::scrollable::Direction::Both {
                vertical: iced::widget::scrollable::Scrollbar::new(),
                horizontal: iced::widget::scrollable::Scrollbar::new(),
            })
            .into()
    }
}
