//! Plot

use iced_aksel as aksel;
use palette::{IntoColor, ShiftHue};
use polars::prelude::{self as pl};

pub type YAxisId = u8;

pub const X_AXIS_ID: &str = "x";
pub const Y_AXIS_IDS: [&str; 1] = ["y0"];
pub const LOG_AXIS_BASE: f64 = 10.0;

#[derive(Debug, Clone, Copy)]
pub enum AxisScale {
    Linear,
    Log,
}

impl Default for AxisScale {
    fn default() -> Self {
        Self::Linear
    }
}

#[derive(Clone, Debug)]
pub enum XAxisValues {
    /// Use dataframe index as axis values.
    Index,
    /// Use a column from the dataframe as axis values.
    /// Value is the name of the column.
    Series(String),
}

impl Default for XAxisValues {
    fn default() -> Self {
        Self::Index
    }
}

#[derive(Default)]
struct XAxis {
    scale: AxisScale,
    values: XAxisValues,
}

#[derive(Debug, Clone)]
struct YAxis {
    id: YAxisId,
    scale: AxisScale,
    position: aksel::axis::Position,
    traces: trace::TraceGroup,
}

impl YAxis {
    pub fn new(id: YAxisId) -> Self {
        Self {
            id,
            scale: Default::default(),
            position: aksel::axis::Position::Left,
            traces: trace::TraceGroup::default(),
        }
    }

    pub fn aksel_id(&self) -> &'static str {
        let idx = self.id as usize;
        Y_AXIS_IDS[idx]
    }

    pub fn position_left(&mut self) {
        self.position = aksel::axis::Position::Left;
    }

    pub fn position_right(&mut self) {
        self.position = aksel::axis::Position::Right;
    }

    pub fn scale_linear(&mut self) {
        self.scale = AxisScale::Linear;
    }

    pub fn scale_log(&mut self) {
        self.scale = AxisScale::Log;
    }

    pub fn add_trace(&mut self, column: impl Into<String>) -> trace::TraceId {
        self.traces.add_trace(column)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Axis {
    X,
    Y(YAxisId),
}

#[derive(Debug, Clone)]
pub enum Message {
    SetTitle(String),
    UpdateXAxisValues(XAxisValues),
    UpdateTraceGroup {
        axis: YAxisId,
        message: trace::GroupMessage,
    },
}

pub struct Options {
    x_axis: XAxis,
    y_axes: Vec<YAxis>,
}

impl Options {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn x_axis(&mut self, x_axis: impl Into<String>) -> &mut Self {
        self.x_axis.values = XAxisValues::Series(x_axis.into());
        self
    }

    pub fn x_axis_log(&mut self) -> &mut Self {
        self.x_axis.scale = AxisScale::Log;
        self
    }

    pub fn new_y_axis(&mut self) -> YAxisId {
        let id = self.y_axes.iter().map(|ax| ax.id).max().unwrap() + 1;
        self.y_axes.push(YAxis::new(id));
        id
    }

    pub fn add_trace(&mut self, y_axis: YAxisId, column: impl Into<String>) -> &mut Self {
        let ax = self.y_axes.iter_mut().find(|ax| ax.id == y_axis).unwrap();
        ax.add_trace(column);
        self
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            x_axis: Default::default(),
            y_axes: vec![YAxis::new(0)],
        }
    }
}

pub struct State {
    chart: aksel::State<&'static str, f64>,
    df: pl::DataFrame,
    title: String,
    x_axis: XAxis,
    y_axes: Vec<YAxis>,
}

impl State {
    pub fn new(dataframe: pl::DataFrame, options: Options) -> Result<Self, ()> {
        let Options { x_axis, y_axes } = options;

        let mut chart = aksel::State::new();
        chart.set_axis(X_AXIS_ID, x_axis_to_aksel(&x_axis, &dataframe));

        for axis in y_axes.iter() {
            let id = axis.aksel_id();
            let axis = y_axis_to_aksel(axis, &dataframe);
            chart.set_axis(id, axis);
        }

        Ok(Self {
            chart,
            df: dataframe,
            title: "".to_string(),
            x_axis,
            y_axes,
        })
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::SetTitle(_) => todo!(),
            Message::UpdateXAxisValues(value) => self.update_x_axis_values(value),
            Message::UpdateTraceGroup { axis, message } => self.update_trace_group(axis, message),
        }
    }

    fn update_x_axis_values(&mut self, values: XAxisValues) -> iced::Task<Message> {
        // TODO: account for logarithmic scale
        self.x_axis.values = values;
        self.chart
            .set_axis(X_AXIS_ID, x_axis_to_aksel(&self.x_axis, &self.df));

        iced::Task::none()
    }

    fn update_trace_group(
        &mut self,
        axis: YAxisId,
        message: trace::GroupMessage,
    ) -> iced::Task<Message> {
        match message {
            trace::GroupMessage::BoundsChanged => {
                let ax = self.y_axis(axis).expect("axis should exist");
                self.chart
                    .set_axis(Y_AXIS_IDS[ax.id as usize], y_axis_to_aksel(&ax, &self.df));
                iced::Task::none()
            }
            trace::GroupMessage::UpdateTrace {
                message: ref trace_message,
                ..
            } => {
                let ax = self.y_axis_mut(axis).expect("axis should exist");
                let bounds_change_task = match trace_message {
                    trace::TraceMessage::ColumnChanged => {
                        iced::Task::done(Message::UpdateTraceGroup {
                            axis,
                            message: trace::GroupMessage::BoundsChanged,
                        })
                    }
                    _ => iced::Task::none(),
                };

                iced::Task::batch([
                    ax.traces
                        .update(message)
                        .map(move |message| Message::UpdateTraceGroup { axis, message }),
                    bounds_change_task,
                ])
            }
            _ => {
                let ax = self.y_axis_mut(axis).expect("axis should exist");
                ax.traces
                    .update(message)
                    .map(move |message| Message::UpdateTraceGroup { axis, message })
            }
        }
    }

    fn y_axis(&self, axis: YAxisId) -> Option<&YAxis> {
        self.y_axes.iter().find(|ax| ax.id == axis)
    }

    fn y_axis_mut(&mut self, axis: YAxisId) -> Option<&mut YAxis> {
        self.y_axes.iter_mut().find(|ax| ax.id == axis)
    }
}

impl State {
    pub fn view(&self) -> iced::Element<'_, Message> {
        let plot = aksel::Chart::new(&self.chart).plot_data(self, X_AXIS_ID, Y_AXIS_IDS[0]);

        let axes_controls = self.axes_controls();
        iced::widget::column![plot, axes_controls].into()
    }

    fn axes_controls(&self) -> iced::Element<'_, Message> {
        let columns = self
            .df
            .schema()
            .iter()
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>();

        let yaxis_groups = self.y_axes.iter().map(|axis| {
            axis.traces
                .view(columns.clone())
                .map(|message| Message::UpdateTraceGroup {
                    axis: axis.id,
                    message: message,
                })
        });

        let columns = std::iter::once("".to_string())
            .chain(columns.clone())
            .collect::<Vec<_>>();
        let pl_xaxis = iced::widget::pick_list(
            columns,
            match &self.x_axis.values {
                XAxisValues::Index => None,
                XAxisValues::Series(column) => Some(column.clone()),
            },
            |selection| {
                let values = if selection.is_empty() {
                    XAxisValues::Index
                } else {
                    XAxisValues::Series(selection)
                };

                Message::UpdateXAxisValues(values)
            },
        )
        .placeholder("<index>");
        let pl_xaxis = iced::widget::row![iced::widget::text("x-axis"), pl_xaxis].into();

        iced::widget::column(std::iter::once(pl_xaxis).chain(yaxis_groups)).into()
    }
}

enum TraceColor {
    Single(iced::Color),
    List(Vec<iced::Color>),
}

impl State {
    fn draw_axis(&self, plot: &mut aksel::Plot<f64>, base_color: iced::Color, axis: &YAxis) {
        let num_traces = axis.traces.len();
        let [r, g, b, a] = base_color.into_linear();
        let base_color_lch: palette::oklch::Oklcha =
            palette::rgb::Srgba::<f32>::from_linear(palette::LinSrgba::new(r, g, b, a))
                .into_color();

        let trace_color_shift = 180.0 / num_traces as f32;
        for (idx, trace) in axis.traces.iter().enumerate() {
            let trace_color = base_color_lch
                .clone()
                .shift_hue(trace_color_shift * idx as f32);

            let color = match trace.color() {
                trace::Color::Default => {
                    let color: palette::LinSrgba = trace_color.into_color();
                    let (r, g, b, a) = color.into_components();
                    let color = iced::Color::from_linear_rgba(r, g, b, a);
                    TraceColor::Single(color)
                }
                trace::Color::Column(column) => {
                    let colors = self.df.column(column).unwrap();
                    let (min, max) = column_minmax_f64(colors);
                    let colors = column_to_values_f64(colors);

                    let range = max - min;
                    let colors = colors
                        .into_iter()
                        .map(|value| (value - min) / range)
                        .map(|shift| trace_color.shift_hue(shift as f32 * 180.0))
                        .map(|color| {
                            let color: palette::LinSrgba = color.into_color();
                            let (r, g, b, a) = color.into_components();
                            iced::Color::from_linear_rgba(r, g, b, a)
                        })
                        .collect::<Vec<_>>();

                    TraceColor::List(colors)
                }
                trace::Color::Custom(_) => todo!(),
            };

            self.draw_trace(plot, color, trace);
        }
    }

    fn draw_trace(&self, plot: &mut aksel::Plot<f64>, color: TraceColor, trace: &trace::Trace) {
        let column_label = trace.column();
        let y = self.df.column(column_label).unwrap();
        let y = column_to_values_f64(y);

        let x = match &self.x_axis.values {
            XAxisValues::Series(column) => {
                let x = self.df.column(column).unwrap();
                column_to_values_f64(x)
            }
            XAxisValues::Index => (0..self.df.height()).map(|x| x as f64).collect::<Vec<_>>(),
        };

        let colors = match color {
            TraceColor::Single(color) => vec![color; self.df.height()],
            TraceColor::List(colors) => colors,
        };

        let points = itertools::izip!(x, y, colors);
        for (x, y, color) in points {
            plot.add_shape(
                aksel::shape::Ellipse::new(
                    aksel::PlotPoint::new(x, y),
                    aksel::Measure::Screen(1.0),
                    aksel::Measure::Screen(1.0),
                )
                .fill(color),
            );
        }
    }
}

impl aksel::PlotData<f64> for State {
    fn draw(&self, plot: &mut aksel::Plot<f64>, theme: &iced::advanced::graphics::core::Theme) {
        for axis in self.y_axes.iter() {
            let base_color = theme.palette().primary;
            self.draw_axis(plot, base_color, axis);
        }
    }
}

mod trace {
    use polars::prelude as pl;

    use crate::icon;

    pub type TraceId = u8;

    #[derive(Clone, Debug)]
    pub enum GroupMessage {
        UpdateTrace {
            trace: TraceId,
            message: TraceMessage,
        },
        BoundsChanged,
        AddTrace,
        TraceAdded,
        RemoveTrace(TraceId),
        TraceRemoved,
    }

    #[derive(Debug, Clone, derive_more::Deref)]
    pub struct TraceGroup {
        traces: Vec<Trace>,
    }

    impl TraceGroup {
        pub fn view(&self, columns: Vec<String>) -> iced::Element<'_, GroupMessage> {
            let btn_add_trace = iced::widget::button(icon::plus()).on_press(GroupMessage::AddTrace);
            let title = iced::widget::column![iced::widget::text("y-axis"), btn_add_trace];
            let allow_remove_trace = self.traces.len() > 1;

            let traces = self.traces.iter().map(|trace| {
                trace
                    .view(columns.clone(), allow_remove_trace)
                    .map(|message| GroupMessage::UpdateTrace {
                        trace: trace.id,
                        message,
                    })
            });
            iced::widget::row![title, iced::widget::column(traces)].into()
        }

        pub fn update(&mut self, message: GroupMessage) -> iced::Task<GroupMessage> {
            match message {
                GroupMessage::UpdateTrace { trace, message } => match message {
                    TraceMessage::Remove => iced::Task::done(GroupMessage::RemoveTrace(trace)),
                    _ => {
                        let tr = self.get_trace_mut(trace).expect("trace should exist");
                        tr.update(message)
                            .map(move |message| GroupMessage::UpdateTrace { trace, message })
                    }
                },
                GroupMessage::BoundsChanged => iced::Task::none(),
                GroupMessage::AddTrace => {
                    let column = self
                        .traces
                        .last()
                        .expect("trace group can not be empty")
                        .column
                        .clone();
                    self.add_trace(column);
                    iced::Task::done(GroupMessage::TraceAdded)
                }
                GroupMessage::TraceAdded => iced::Task::none(),
                GroupMessage::RemoveTrace(trace_id) => {
                    assert!(self.traces.len() > 1, "last trace should not be removable");

                    self.traces.retain(|trace| trace.id != trace_id);
                    iced::Task::done(GroupMessage::TraceRemoved)
                }
                GroupMessage::TraceRemoved => iced::Task::done(GroupMessage::BoundsChanged),
            }
        }

        pub fn get_trace_mut(&mut self, id: TraceId) -> Option<&mut Trace> {
            self.traces.iter_mut().find(|trace| trace.id == id)
        }

        pub fn add_trace(&mut self, column: impl Into<String>) -> TraceId {
            let id = self
                .traces
                .iter()
                .map(|trace| trace.id + 1)
                .max()
                .unwrap_or_default();
            self.traces.push(Trace::new(id, column));
            id
        }

        pub fn minmax_f64(&self, dataframe: &pl::DataFrame) -> (f64, f64) {
            let mut min = f64::MAX;
            let mut max = f64::MIN;
            for trace in self.traces.iter() {
                let (tmin, tmax) = trace.minmax_f64(dataframe);
                if tmin < min {
                    min = tmin;
                }
                if tmax > max {
                    max = tmax;
                }
            }

            (min, max)
        }
    }

    impl Default for TraceGroup {
        fn default() -> Self {
            Self {
                traces: Default::default(),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub enum Color {
        Default,
        Column(String),
        Custom(i32),
    }

    impl Color {
        fn to_picklist_option(&self) -> String {
            match self {
                Color::Default => "".to_string(),
                Color::Column(column) => column.clone(),
                Color::Custom(_) => "<custom>".to_string(),
            }
        }
    }

    impl Default for Color {
        fn default() -> Self {
            Self::Default
        }
    }

    #[derive(Clone, Debug)]
    pub enum TraceMessage {
        /// Change the data column.
        ColumnChange(String),
        ColumnChanged,
        ColorChange(Color),
        /// Remove trace.
        Remove,
    }

    // TODO: Add marker option.
    #[derive(Clone, Debug)]
    pub struct Trace {
        id: TraceId,
        /// Column name in the dataframe.
        column: String,
        color: Color,
        /// Aplha (opacity) channel.
        /// Between 0 (transparent) and 1 (opaque).
        alpha: f64,
    }

    impl Trace {
        pub fn new(id: TraceId, column: impl Into<String>) -> Self {
            Self {
                id,
                column: column.into(),
                color: Default::default(),
                alpha: 1.0,
            }
        }

        pub fn column(&self) -> &String {
            &self.column
        }

        pub fn color(&self) -> &Color {
            &self.color
        }

        /// Set the alpha (opacity) channel value.
        /// Value is clamped between 0 (transparent) and 1 (opaque).
        pub fn set_alpha(&mut self, alpha: f64) {
            if alpha < 0.0 {
                self.alpha = 0.0;
            } else if alpha > 1.0 {
                self.alpha = 1.0;
            } else {
                self.alpha = alpha
            }
        }

        pub fn minmax_f64(&self, dataframe: &pl::DataFrame) -> (f64, f64) {
            let mut min = f64::MAX;
            let mut max = f64::MIN;
            let column = dataframe.column(&self.column).unwrap();
            let (cmin, cmax) = super::column_minmax_f64(column);
            if cmin < min {
                min = cmin;
            }
            if cmax > max {
                max = cmax;
            }

            (min, max)
        }
    }

    impl Trace {
        pub fn update(&mut self, message: TraceMessage) -> iced::Task<TraceMessage> {
            match message {
                TraceMessage::ColumnChange(column) => {
                    self.column = column;
                    return iced::Task::done(TraceMessage::ColumnChanged);
                }
                TraceMessage::ColumnChanged => iced::Task::none(),
                TraceMessage::ColorChange(color) => {
                    self.color = color;
                    iced::Task::none()
                }
                TraceMessage::Remove => iced::Task::none(),
            }
        }

        pub fn view(
            &self,
            columns: Vec<String>,
            removable: bool,
        ) -> iced::Element<'_, TraceMessage> {
            assert!(columns.contains(&self.column));
            let pl_column = iced::widget::pick_list(
                columns.clone(),
                Some(self.column.clone()),
                TraceMessage::ColumnChange,
            );

            let color_options = std::iter::once("".to_string())
                .chain(columns.into_iter())
                .chain(std::iter::once("<custom>".to_string()))
                .collect::<Vec<_>>();
            let pl_color = iced::widget::pick_list(
                color_options,
                Some(self.color.to_picklist_option()),
                |selected| {
                    if selected == "" {
                        TraceMessage::ColorChange(Color::Default)
                    } else if selected == "<custom>" {
                        TraceMessage::ColorChange(Color::Custom(50))
                    } else {
                        TraceMessage::ColorChange(Color::Column(selected))
                    }
                },
            )
            .placeholder("<default>");

            let btn_remove = removable
                .then_some(iced::widget::button(icon::minus()).on_press(TraceMessage::Remove));

            iced::widget::row![pl_column, pl_color, btn_remove].into()
        }
    }
}

fn x_axis_to_aksel(axis: &XAxis, df: &pl::DataFrame) -> aksel::Axis<f64> {
    const POSITION: aksel::axis::Position = aksel::axis::Position::Bottom;

    match &axis.values {
        XAxisValues::Index => {
            aksel::Axis::new(aksel::scale::Linear::new(0.0, df.height() as f64), POSITION)
        }
        XAxisValues::Series(name) => {
            let column = df.column(name).unwrap();
            let (min, max) = column_minmax_f64(column);
            match axis.scale {
                AxisScale::Linear => {
                    aksel::Axis::new(aksel::scale::Linear::new(min, max), POSITION)
                }
                AxisScale::Log => aksel::Axis::new(aksel::scale::Linear::new(min, max), POSITION),
            }
        }
    }
}

fn y_axis_to_aksel(axis: &YAxis, df: &pl::DataFrame) -> aksel::Axis<f64> {
    let (min, max) = axis.traces.minmax_f64(df);
    match axis.scale {
        AxisScale::Linear => aksel::Axis::new(aksel::scale::Linear::new(min, max), axis.position),
        AxisScale::Log => aksel::Axis::new(aksel::scale::Linear::new(min, max), axis.position),
    }
}

fn column_minmax_f64(column: &pl::Column) -> (f64, f64) {
    let values = column.as_series().unwrap();
    match column.dtype() {
        pl::DataType::UInt8 => {
            let min = values.min::<u8>().unwrap().unwrap();
            let max = values.max::<u8>().unwrap().unwrap();
            (min as f64, max as f64)
        }
        pl::DataType::Float64 => {
            let min = values.min::<f64>().unwrap().unwrap();
            let max = values.max::<f64>().unwrap().unwrap();
            (min, max)
        }
        _ => todo!(),
    }
}

fn column_to_values_f64(column: &pl::Column) -> Vec<f64> {
    match column.dtype() {
        pl::DataType::Float64 => column
            .f64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        pl::DataType::UInt8 => column
            .u8()
            .unwrap()
            .into_no_null_iter()
            .map(|v| v as f64)
            .collect::<Vec<_>>(),
        _ => todo!(),
    }
}
