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

#[derive(Debug)]
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
        // TODO: Headers should be sticky
        let df_guard = self.df.read().expect("dataframe should be readable");
        let df = (*df_guard).clone();
        let highlight = self.highlight;
        let height = df.height();
        let columns_data: Vec<(String, pl::Column)> = df
            .schema()
            .iter()
            .map(|(name, _)| (name.to_string(), df.column(name).unwrap().clone()))
            .collect();
        drop(df);

        let columns: Vec<_> = columns_data
            .into_iter()
            .map(|(name, col)| {
                widget::table::column(widget::text(name.clone()), move |idx: usize| {
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
                    if let Some(hidx) = highlight {
                        if idx == hidx {
                            text = text.color(theme.palette().success);
                        }
                    }
                    text
                })
            })
            .collect();

        let table = widget::table::Table::new(columns, 0..height);
        widget::scrollable(table)
            .direction(iced::widget::scrollable::Direction::Both {
                vertical: iced::widget::scrollable::Scrollbar::new(),
                horizontal: iced::widget::scrollable::Scrollbar::new(),
            })
            .into()
    }
}
