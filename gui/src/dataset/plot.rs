//! Plot
use iced_aksel as aksel;
use palette::IntoColor;
use polars::prelude::{self as pl};

type PlotValue = f64;
pub type ValueAxisId = u8;

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
pub enum IndexValues {
    /// Use dataframe index as axis values.
    Index,
    /// Use a column from the dataframe as axis values.
    /// Value is the name of the column.
    Series(String),
}

impl IndexValues {
    pub fn take(&mut self) -> Option<String> {
        match std::mem::replace(self, Self::Index) {
            IndexValues::Index => None,
            IndexValues::Series(label) => Some(label),
        }
    }

    pub fn insert(&mut self, value: impl Into<String>) -> Option<String> {
        match std::mem::replace(self, Self::Series(value.into())) {
            IndexValues::Index => None,
            IndexValues::Series(label) => Some(label),
        }
    }
}

impl Default for IndexValues {
    fn default() -> Self {
        Self::Index
    }
}

#[derive(Default, Debug, Clone)]
pub struct IndexAxis {
    scale: AxisScale,
    values: IndexValues,
}

#[derive(Debug, Clone)]
pub struct ValueAxis {
    id: ValueAxisId,
    scale: AxisScale,
    position: aksel::axis::Position,
    traces: trace::TraceGroup,
}

impl ValueAxis {
    pub fn new(id: ValueAxisId) -> Self {
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

#[derive(Clone, Debug, derive_more::From)]
pub enum YAxisKind {
    Index(IndexAxis),
    Values(Vec<ValueAxis>),
}

pub struct Options<Y> {
    x: IndexAxis,
    y: Y,
}

impl Options<Vec<ValueAxis>> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn x_axis(&mut self, x_axis: impl Into<String>) -> &mut Self {
        self.x.values = IndexValues::Series(x_axis.into());
        self
    }

    pub fn x_axis_log(&mut self) -> &mut Self {
        self.x.scale = AxisScale::Log;
        self
    }

    pub fn new_y_axis(&mut self) -> ValueAxisId {
        let id = self
            .y
            .iter()
            .map(|ax| ax.id)
            .max()
            .map(|id| id + 1)
            .unwrap_or_default();

        self.y.push(ValueAxis::new(id));
        id
    }

    pub fn add_trace(&mut self, y_axis: ValueAxisId, column: impl Into<String>) -> &mut Self {
        let ax = self
            .y
            .iter_mut()
            .find(|ax| ax.id == y_axis)
            .expect(&format!("invalid y axis: {y_axis}"));

        ax.add_trace(column);
        self
    }
}

impl Options<IndexAxis> {
    pub fn new_index_y() -> Self {
        Self {
            x: Default::default(),
            y: Default::default(),
        }
    }

    pub fn x_axis(&mut self, x_axis: impl Into<String>) -> &mut Self {
        self.x.values = IndexValues::Series(x_axis.into());
        self
    }

    pub fn x_axis_log(&mut self) -> &mut Self {
        self.x.scale = AxisScale::Log;
        self
    }
}

impl Default for Options<Vec<ValueAxis>> {
    fn default() -> Self {
        Options {
            x: Default::default(),
            y: Default::default(),
        }
    }
}

pub struct Settings {
    x: IndexAxis,
    y: YAxisKind,
}

impl<Y> From<Options<Y>> for Settings
where
    Y: Into<YAxisKind>,
{
    fn from(value: Options<Y>) -> Self {
        let Options { x, y } = value;

        Self { x, y: y.into() }
    }
}

#[derive(Debug, Clone, derive_more::From)]
pub enum Message {
    SetTitle(String),
    DataframeChange(pl::DataFrame),
    UpdateXAxisValues(IndexValues),
    XAxisValuesUpdated,
    #[from]
    Data(data::Message),
    #[from]
    YAxis(MessageYAxis),
}

#[derive(Debug, Clone, derive_more::From)]
pub enum MessageYAxis {
    TraceGroup {
        axis: ValueAxisId,
        message: trace::GroupMessage,
    },
}

pub struct State {
    default: Settings,
    chart: aksel::State<&'static str, PlotValue>,
    data: aksel::Cached<data::State>,
    title: String,
}

impl State {
    pub fn new<Y>(dataframe: pl::DataFrame, options: Options<Y>) -> Result<Self, ()>
    where
        Y: Into<YAxisKind>,
    {
        let default: Settings = options.into();
        let mut chart = aksel::State::new();
        chart.set_axis(X_AXIS_ID, index_axis_to_aksel(&default.x, &dataframe));

        match &default.y {
            YAxisKind::Index(index_axis) => todo!(),
            YAxisKind::Values(items) => {
                for axis in items.iter() {
                    let id = axis.aksel_id();
                    let axis = value_axis_to_aksel(axis, &dataframe);
                    chart.set_axis(id, axis);
                }
            }
        }

        let data = data::State::new(dataframe, default.x.clone(), default.y.clone());
        Ok(Self {
            default,
            chart,
            data: aksel::Cached::new(data),
            title: "".to_string(),
        })
    }

    pub fn record_idx_by_point_id(&self, id: &aksel::interaction::Id) -> Option<usize> {
        self.data.get().record_idx_by_point_id(id)
    }
}

impl State {
    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::SetTitle(_) => todo!(),
            Message::UpdateXAxisValues(value) => self.update_x_axis_values(value),
            Message::XAxisValuesUpdated => self.rescale_xaxis(),
            Message::DataframeChange(dataframe) => self.dataframe_change(dataframe),
            Message::Data(message) => self.update_data(message),
            Message::YAxis(MessageYAxis::TraceGroup { axis, message }) => {
                self.update_trace_group(axis, message)
            }
        }
    }

    fn update_x_axis_values(&mut self, values: IndexValues) -> iced::Task<Message> {
        // TODO: account for logarithmic scale
        let data = self.data.edit();
        data.x_axis.values = values;
        iced::Task::done(Message::XAxisValuesUpdated)
    }

    fn rescale_xaxis(&mut self) -> iced::Task<Message> {
        let data = self.data.get();
        let axis = index_axis_to_aksel(&data.x_axis, &data.df);
        self.chart.set_axis(X_AXIS_ID, axis);
        iced::Task::none()
    }

    fn update_trace_group(
        &mut self,
        axis: ValueAxisId,
        message: trace::GroupMessage,
    ) -> iced::Task<Message> {
        match message {
            trace::GroupMessage::BoundsChanged => {
                let data = self.data.get();
                let ax = data.y_axis(axis).expect("axis should exist");
                self.chart.set_axis(
                    Y_AXIS_IDS[ax.id as usize],
                    value_axis_to_aksel(&ax, &data.df),
                );
                iced::Task::none()
            }
            trace::GroupMessage::UpdateTrace {
                message: ref trace_message,
                ..
            } => {
                let data = self.data.edit();
                let ax = data.y_axis_mut(axis).expect("axis should exist");
                let bounds_change_task = match trace_message {
                    trace::TraceMessage::ColumnChanged => iced::Task::done(
                        MessageYAxis::TraceGroup {
                            axis,
                            message: trace::GroupMessage::BoundsChanged,
                        }
                        .into(),
                    ),
                    _ => iced::Task::none(),
                };

                iced::Task::batch([
                    ax.traces
                        .update(message)
                        .map(move |message| MessageYAxis::TraceGroup { axis, message }.into()),
                    bounds_change_task,
                ])
            }
            _ => {
                let data = self.data.edit();
                let ax = data.y_axis_mut(axis).expect("axis should exist");
                ax.traces
                    .update(message)
                    .map(move |message| MessageYAxis::TraceGroup { axis, message }.into())
            }
        }
    }

    fn dataframe_change(&mut self, dataframe: pl::DataFrame) -> iced::Task<Message> {
        let mut tasks = Vec::new();

        let data = self.data.edit();
        data.update_dataframe(dataframe);
        let columns = data
            .df
            .schema()
            .iter_names()
            .map(|name| name.to_string())
            .collect::<Vec<_>>();

        if let IndexValues::Series(label) = &data.x_axis.values {
            if !columns.contains(label) {
                if let IndexValues::Series(default) = &self.default.x.values {
                    data.x_axis.values.insert(default.clone());
                } else {
                    data.x_axis.values.take();
                }
            }
        }
        tasks.push(iced::Task::done(Message::XAxisValuesUpdated));

        match &mut data.y_axis {
            YAxisKind::Index(index_axis) => todo!(),
            YAxisKind::Values(items) => {
                // for axis in items.iter_mut() {
                //     axis.traces.retain(|trace| columns.contains(trace.column()));

                //     for trace in axis.traces.iter_mut() {
                //         if let trace::Color::Column(column) = trace.color() {
                //             if !columns.contains(&column) {
                //                 trace.set_color(trace::Color::Default);
                //             }
                //         }
                //     }

                //     if axis.traces.len() == 0 {
                //         let mut add = vec![];
                //         if let Some(default) = self.default.y.iter().find(|ax| ax.id == axis.id) {
                //             for trace in default.traces.iter() {
                //                 if columns.contains(trace.column()) {
                //                     add.push(trace.clone());
                //                 }
                //             }
                //         }
                //         if add.len() == 0 {
                //             axis.add_trace(&columns[0]);
                //         } else {
                //             axis.traces.extend(add);
                //         }
                //     }

                //     tasks.extend([
                //         iced::Task::done(
                //             MessageYAxis::TraceGroup {
                //                 axis: axis.id,
                //                 message: trace::GroupMessage::TracesUpdated,
                //             }
                //             .into(),
                //         ),
                //         iced::Task::done(
                //             MessageYAxis::TraceGroup {
                //                 axis: axis.id,
                //                 message: trace::GroupMessage::BoundsChanged,
                //             }
                //             .into(),
                //         ),
                //     ]);
                // }
            }
        }

        iced::Task::batch(tasks)
    }

    fn update_data(&mut self, message: data::Message) -> iced::Task<Message> {
        match message {
            data::Message::ChartDragged(delta) => {
                // TODO: Account for multiple y axes
                self.chart
                    .pan_axes(X_AXIS_ID, Y_AXIS_IDS[0], delta.x, delta.y);
                iced::Task::none()
            }
            data::Message::ChartScrolled(aksel::ScrollEvent {
                delta, position, ..
            }) => {
                let zoom_factor = match delta {
                    iced::mouse::ScrollDelta::Lines { y, .. }
                    | iced::mouse::ScrollDelta::Pixels { y, .. } => {
                        if y > 0.0 {
                            1.1
                        } else {
                            0.9
                        }
                    }
                };

                // TODO: Account for multiple y axes
                self.chart
                    .axis_mut(&X_AXIS_ID)
                    .zoom(zoom_factor, Some(position.x));
                self.chart
                    .axis_mut(&Y_AXIS_IDS[0])
                    .zoom(zoom_factor, Some(position.y));
                iced::Task::none()
            }
            _ => self.data.edit().update(message).map(Message::Data),
        }
    }
}

impl State {
    pub fn view(&self) -> iced::Element<'_, Message> {
        let plot = aksel::Chart::new(&self.chart)
            .marker(
                &X_AXIS_ID,
                aksel::axis::MarkerPosition::Cursor,
                axis_renderer_marker,
            )
            .marker(
                &Y_AXIS_IDS[0],
                aksel::axis::MarkerPosition::Cursor,
                axis_renderer_marker,
            )
            .plot_data(self.data.get(), X_AXIS_ID, Y_AXIS_IDS[0])
            .on_drag(|event: aksel::DragEvent<aksel::Delta>| {
                (event.button_held == iced::mouse::Button::Left)
                    .then_some(data::Message::ChartDragged(event.delta).into())
            })
            .on_scroll(data::Message::ChartScrolled);

        let plot: iced::Element<'_, _> = plot.into();
        let plot = plot.map(Message::Data);
        let axes_controls = self.axes_controls();

        iced::widget::column![plot, axes_controls].into()
    }

    fn axes_controls(&self) -> iced::Element<'_, Message> {
        let data = self.data.get();
        let pl_xaxis = self.pl_index_axis(&data.x_axis.values, Message::UpdateXAxisValues);
        let pl_xaxis = iced::widget::row![iced::widget::text("x-axis"), pl_xaxis].into();

        let yaxis = match &data.y_axis {
            YAxisKind::Index(axis) => todo!(),
            YAxisKind::Values(items) => items
                .iter()
                .map(|axis| self.pl_values_axis(axis))
                .collect::<Vec<_>>(),
        };

        iced::widget::column(std::iter::once(pl_xaxis).chain(yaxis)).into()
    }

    fn pl_index_axis<'a, F>(
        &'a self,
        selected: &'a IndexValues,
        message: F,
    ) -> iced::Element<'a, Message>
    where
        F: Fn(IndexValues) -> Message + 'a,
    {
        let data = self.data.get();
        let columns = data
            .df
            .schema()
            .iter()
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>();
        let columns = std::iter::once("".to_string())
            .chain(columns)
            .collect::<Vec<_>>();

        iced::widget::pick_list(
            columns,
            match selected {
                IndexValues::Index => None,
                IndexValues::Series(column) => Some(column.clone()),
            },
            move |selection| {
                let values = if selection.is_empty() {
                    IndexValues::Index
                } else {
                    IndexValues::Series(selection)
                };

                message(values)
            },
        )
        .placeholder("<index>")
        .into()
    }

    fn pl_values_axis<'a>(&'a self, axis: &'a ValueAxis) -> iced::Element<'a, Message> {
        let data = self.data.get();
        let columns = data
            .df
            .schema()
            .iter()
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>();

        let axis_id = axis.id;
        axis.traces.view(columns.clone()).map(move |message| {
            MessageYAxis::TraceGroup {
                axis: axis_id,
                message,
            }
            .into()
        })
    }
}

fn axis_renderer_marker(ctx: aksel::axis::MarkerContext<PlotValue>) -> Option<aksel::axis::Marker> {
    if !ctx.cursor_on_plot && !ctx.cursor_on_axis {
        return None;
    }

    Some(ctx.marker(format!("{:.02e}", ctx.value)))
}

fn axis_renderer_ticks()
-> impl Fn(aksel::axis::TickContext<PlotValue>) -> aksel::axis::TickResult + 'static {
    move |ctx: aksel::axis::TickContext<PlotValue>| {
        let text = format!("{:.02e}", ctx.tick.value);
        let label = ctx.label(text);

        aksel::axis::TickResult {
            label: Some(label),
            label_badge: Some(ctx.label_badge()),
            tick_line: Some(ctx.tickline()),
            grid_line: Some(ctx.gridline()),
            label_priority: None,
        }
    }
}

mod trace {
    use crate::icon;
    use polars::prelude as pl;

    pub type TraceId = u8;
    pub type MarkerSize = f32;

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
        TracesUpdated,
    }

    #[derive(Debug, Clone, derive_more::Deref, derive_more::DerefMut)]
    pub struct TraceGroup {
        pub(crate) traces: Vec<Trace>,
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
                GroupMessage::TracesUpdated => iced::Task::none(),
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
        MarkerSizeChange(MarkerSize),
        MarkerSizeIncrement,
        MarkerSizeDecrement,
        /// Remove trace.
        Remove,
    }

    #[derive(Clone, Copy, Default, Debug)]
    pub enum Marker {
        #[default]
        Circle,
        Square,
        Triangle,
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
        marker: Marker,
        marker_size: MarkerSize,
    }

    impl Trace {
        pub fn new(id: TraceId, column: impl Into<String>) -> Self {
            Self {
                id,
                column: column.into(),
                color: Default::default(),
                alpha: 1.0,
                marker: Default::default(),
                marker_size: 1.0,
            }
        }

        pub fn id(&self) -> TraceId {
            self.id
        }

        pub fn column(&self) -> &String {
            &self.column
        }

        pub fn color(&self) -> &Color {
            &self.color
        }

        pub fn set_color(&mut self, color: Color) {
            self.color = color;
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

        pub fn marker(&self) -> Marker {
            self.marker
        }

        pub fn marker_size(&self) -> MarkerSize {
            self.marker_size
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
                TraceMessage::MarkerSizeChange(size) => {
                    self.marker_size = size;
                    iced::Task::none()
                }
                TraceMessage::MarkerSizeIncrement => {
                    self.marker_size += 1.0;
                    iced::Task::none()
                }
                TraceMessage::MarkerSizeDecrement => {
                    if self.marker_size > 1.0 {
                        self.marker_size -= 1.0;
                    }
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

            let in_marker_size =
                iced::widget::text_input("Marker size", &self.marker_size.to_string()).on_input(
                    |input| {
                        let value = match input.parse::<MarkerSize>() {
                            Ok(value) => value,
                            Err(err) => {
                                #[cfg(feature = "tracing")]
                                tracing::debug!("could not parse input `{input}`: {err:?}");

                                self.marker_size
                            }
                        };

                        TraceMessage::MarkerSizeChange(value)
                    },
                );
            let up_marker_size =
                iced::widget::button(icon::caret_up()).on_press(TraceMessage::MarkerSizeIncrement);
            let dn_marker_size = iced::widget::button(icon::caret_down())
                .on_press(TraceMessage::MarkerSizeDecrement);
            let btns_marker_size = iced::widget::column![up_marker_size, dn_marker_size];
            let marker_size = iced::widget::row![in_marker_size, btns_marker_size];

            let btn_remove = removable
                .then_some(iced::widget::button(icon::minus()).on_press(TraceMessage::Remove));

            iced::widget::row![pl_column, pl_color, marker_size, btn_remove].into()
        }
    }
}

pub(super) mod data {
    use super::trace;
    use crate::dataset::plot::{color_to_lch, lch_to_color};
    use iced_aksel::{self as aksel, interaction::IntoArea};
    use palette::ShiftHue;
    use polars::prelude as pl;

    #[derive(Debug, Clone)]
    pub enum Message {
        ChartDragged(aksel::Delta),
        ChartScrolled(aksel::ScrollEvent<iced::Point>),
        ShapeEnter {
            point: aksel::interaction::Id,
            event: aksel::EnterEvent,
        },
        ShapeExit,
        PointMouseDown {
            point: aksel::interaction::Id,
            event: aksel::PressEvent<iced::Point>,
        },
    }

    pub struct Points {
        by_idx: Vec<aksel::interaction::Id>,
        by_id: std::collections::HashMap<aksel::interaction::Id, usize>,
    }

    impl Points {
        pub fn new(height: usize) -> Self {
            let mut by_idx = Vec::with_capacity(height);
            let mut by_id = std::collections::HashMap::with_capacity(height);
            for idx in 0..height {
                let id = aksel::interaction::Id::unique();
                by_idx.push(id.clone());
                by_id.insert(id, idx);
            }

            Self { by_idx, by_id }
        }

        pub fn get_by_idx(&self, idx: usize) -> Option<aksel::interaction::Id> {
            self.by_idx.get(idx).cloned()
        }

        pub fn get_by_id(&self, id: &aksel::interaction::Id) -> Option<usize> {
            self.by_id.get(id).cloned()
        }

        pub fn iter_idx(&self) -> impl Iterator<Item = &aksel::interaction::Id> {
            self.by_idx.iter()
        }

        pub fn clear(&mut self) {
            self.by_id.clear();
            self.by_idx.clear();
        }
    }

    pub struct State {
        pub(super) df: pl::DataFrame,
        pub(super) x_axis: super::IndexAxis,
        pub(super) y_axis: super::YAxisKind,
        points: Points,
        pub(super) hovered_id: Option<aksel::interaction::Id>,
        pub(super) selected_id: Option<aksel::interaction::Id>,
    }

    impl State {
        pub fn new(df: pl::DataFrame, x_axis: super::IndexAxis, y_axis: super::YAxisKind) -> Self {
            let points = Points::new(df.height());
            Self {
                df,
                x_axis,
                y_axis,
                points,
                hovered_id: Default::default(),
                selected_id: Default::default(),
            }
        }

        pub fn y_axis(&self, axis: super::ValueAxisId) -> Option<&super::ValueAxis> {
            match &self.y_axis {
                super::YAxisKind::Index(index_axis) => None,
                super::YAxisKind::Values(items) => items.iter().find(|ax| ax.id == axis),
            }
        }

        pub fn y_axis_mut(&mut self, axis: super::ValueAxisId) -> Option<&mut super::ValueAxis> {
            match &mut self.y_axis {
                super::YAxisKind::Index(index_axis) => None,
                super::YAxisKind::Values(items) => items.iter_mut().find(|ax| ax.id == axis),
            }
        }

        pub fn record_idx_by_point_id(&self, id: &aksel::interaction::Id) -> Option<usize> {
            self.points.get_by_id(id)
        }

        pub fn update_dataframe(&mut self, dataframe: pl::DataFrame) {
            let _ = self.hovered_id.take();
            let _ = self.selected_id.take();
            self.points = Points::new(dataframe.height());
            self.df = dataframe;
        }
    }

    impl State {
        pub fn update(&mut self, message: Message) -> iced::Task<Message> {
            match message {
                Message::ChartDragged(_) => unreachable!("handled elsewhere"),
                Message::ChartScrolled(_) => unreachable!("handled elsewhere"),
                Message::ShapeEnter { point, event } => {
                    let _ = self.hovered_id.insert(point);
                    iced::Task::none()
                }
                Message::ShapeExit => {
                    self.selected_id = None;
                    iced::Task::none()
                }
                Message::PointMouseDown { point, event } => iced::Task::none(),
            }
        }
    }

    impl State {
        fn draw_axis(
            &self,
            plot: &mut aksel::Plot<super::PlotValue, Message>,
            base_color: iced::Color,
            axis: &super::ValueAxis,
        ) {
            let num_traces = axis.traces.len();
            assert_ne!(num_traces, 0, "axis traces must not be empty");

            let base_color_lch = color_to_lch(base_color);
            let trace_color_shift = 180.0 / num_traces as f32;
            for (idx, trace) in axis.traces.iter().enumerate() {
                let trace_color = base_color_lch
                    .clone()
                    .shift_hue(trace_color_shift * idx as f32);

                let mut colors = match trace.color() {
                    trace::Color::Default => {
                        let color = lch_to_color(trace_color);
                        vec![color; self.df.height()]
                    }
                    trace::Color::Column(column) => {
                        let colors = self.df.column(&column).expect("color column should exist");
                        let (min, max) = super::column_minmax_f64(colors);
                        let colors = super::column_to_values_f64(colors);

                        let range = max - min;
                        if approx::abs_diff_eq!(range, 0.0) {
                            let color = lch_to_color(trace_color);
                            vec![color; self.df.height()]
                        } else {
                            colors
                                .into_iter()
                                .map(|value| (value - min) / range)
                                .map(|value| {
                                    if value.is_nan() {
                                        palette::Oklch::from_components((0.0, 0.0, 0.0))
                                    } else {
                                        trace_color.shift_hue(value as f32 * 180.0)
                                    }
                                })
                                .map(lch_to_color)
                                .collect::<Vec<_>>()
                        }
                    }
                    trace::Color::Custom(_) => todo!(),
                };

                if let Some(id) = &self.hovered_id {
                    let idx = self.points.get_by_id(id).expect("point should exist");
                    let color = colors.get_mut(idx).expect("color should exist");
                    let lch = color_to_lch(color.clone());
                    let lch = lch.shift_hue(120.0);
                    *color = lch_to_color(lch);
                }

                self.draw_trace(plot, colors, trace);
            }
        }

        fn draw_trace(
            &self,
            plot: &mut aksel::Plot<super::PlotValue, Message>,
            colors: Vec<iced::Color>,
            trace: &trace::Trace,
        ) {
            let column_label = trace.column();
            let y = self.df.column(column_label).unwrap();
            let y = super::column_to_values_f64(y);

            let x = match &self.x_axis.values {
                super::IndexValues::Series(column) => {
                    let x = self.df.column(&column).unwrap();
                    super::column_to_values_f64(x)
                }
                super::IndexValues::Index => (0..self.df.height())
                    .map(|x| x as super::PlotValue)
                    .collect::<Vec<_>>(),
            };

            let mut hovered = None;
            let points = itertools::izip!(self.points.iter_idx(), x, y, colors);
            for (iid, x, y, color) in points {
                if let Some(hovered_id) = &self.hovered_id {
                    if iid == hovered_id {
                        let _ = hovered.insert((iid, x, y, color));
                        continue;
                    }
                }
                Self::draw_point(plot, trace.marker(), iid, x, y, color, trace.marker_size());
            }

            if let Some((id, x, y, color)) = hovered {
                Self::draw_point(plot, trace.marker(), id, x, y, color, trace.marker_size());
            }
        }

        #[inline]
        fn draw_point(
            plot: &mut aksel::Plot<super::PlotValue, Message>,
            marker: trace::Marker,
            id: &aksel::interaction::Id,
            x: super::PlotValue,
            y: super::PlotValue,
            color: iced::Color,
            marker_size: super::trace::MarkerSize,
        ) {
            let marker_size = aksel::Measure::Screen(marker_size);
            let interaction = match marker {
                trace::Marker::Circle => {
                    let shape = aksel::shape::Ellipse::circle(
                        aksel::PlotPoint::new(x, y),
                        marker_size.clone(),
                    )
                    .fill(color);

                    let area = shape.resolve_area(&plot);
                    plot.render(shape);

                    aksel::Interaction::new(area)
                        .on_enter(|point, event| Message::ShapeEnter { point, event })
                        .on_exit(Message::ShapeExit)
                        .on_press(|point, event| Message::PointMouseDown { point, event })
                }
                trace::Marker::Square => {
                    let shape = aksel::shape::Rectangle::centered(
                        aksel::PlotPoint::new(x, y),
                        marker_size.clone(),
                        marker_size.clone(),
                    )
                    .fill(color);

                    let area = shape.resolve_area(&plot);
                    plot.render(shape);

                    aksel::Interaction::new(area)
                        .on_enter(|point, event| Message::ShapeEnter { point, event })
                        .on_exit(Message::ShapeExit)
                        .on_press(|point, event| Message::PointMouseDown { point, event })
                }
                trace::Marker::Triangle => {
                    let shape = aksel::shape::Triangle::centered(
                        aksel::PlotPoint::new(x, y),
                        marker_size.clone(),
                        marker_size.clone(),
                    )
                    .fill(color);

                    let area = shape.resolve_area(&plot);
                    plot.render(shape);

                    aksel::Interaction::new(area)
                        .on_enter(|point, event| Message::ShapeEnter { point, event })
                        .on_exit(Message::ShapeExit)
                        .on_press(|point, event| Message::PointMouseDown { point, event })
                }
            };

            plot.push_interaction(id.clone(), interaction);
        }
    }

    impl aksel::PlotData<super::PlotValue, Message> for State {
        fn draw(
            &self,
            plot: &mut aksel::Plot<super::PlotValue, Message>,
            theme: &iced::advanced::graphics::core::Theme,
        ) {
            match &self.y_axis {
                super::YAxisKind::Index(index_axis) => {
                    todo!()
                }
                super::YAxisKind::Values(items) => {
                    for axis in items.iter() {
                        let base_color = theme.palette().primary;
                        self.draw_axis(plot, base_color, axis);
                    }
                }
            }
        }
    }
}

fn index_axis_to_aksel(axis: &IndexAxis, df: &pl::DataFrame) -> aksel::Axis<PlotValue> {
    const POSITION: aksel::axis::Position = aksel::axis::Position::Bottom;

    match &axis.values {
        IndexValues::Index => aksel::Axis::new(
            aksel::scale::Linear::new(0.0, df.height() as PlotValue),
            POSITION,
        )
        .with_tick_renderer(axis_renderer_ticks()),
        IndexValues::Series(name) => {
            let column = df.column(name).unwrap();
            let (min, max) = column_minmax_f64(column);
            match axis.scale {
                AxisScale::Linear => {
                    aksel::Axis::new(aksel::scale::Linear::new(min, max), POSITION)
                        .with_tick_renderer(axis_renderer_ticks())
                }
                AxisScale::Log => aksel::Axis::new(aksel::scale::Linear::new(min, max), POSITION)
                    .with_tick_renderer(axis_renderer_ticks()),
            }
        }
    }
}

#[inline]
fn value_axis_to_aksel(axis: &ValueAxis, df: &pl::DataFrame) -> aksel::Axis<PlotValue> {
    let (min, max) = axis.traces.minmax_f64(df);
    match axis.scale {
        AxisScale::Linear => aksel::Axis::new(aksel::scale::Linear::new(min, max), axis.position)
            .with_tick_renderer(axis_renderer_ticks()),

        AxisScale::Log => aksel::Axis::new(aksel::scale::Linear::new(min, max), axis.position)
            .with_tick_renderer(axis_renderer_ticks()),
    }
}

#[inline]
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
        pl::DataType::Int64 => {
            let min = values.min::<i64>().unwrap().unwrap();
            let max = values.max::<i64>().unwrap().unwrap();
            (min as f64, max as f64)
        }
        pl::DataType::Int128 => {
            let min = values.min::<i128>().unwrap().unwrap();
            let max = values.max::<i128>().unwrap().unwrap();
            (min as f64, max as f64)
        }
        _ => todo!(),
    }
}

#[inline]
fn column_to_values_f64(column: &pl::Column) -> Vec<f64> {
    match column.dtype() {
        pl::DataType::Float64 => column.f64().unwrap().into_no_null_iter().collect(),
        pl::DataType::UInt8 => column
            .u8()
            .unwrap()
            .into_no_null_iter()
            .map(|v| v as f64)
            .collect(),
        pl::DataType::Int64 => column
            .i64()
            .unwrap()
            .into_no_null_iter()
            .map(|v| v as f64)
            .collect(),
        kind => todo!("{kind:?}"),
    }
}

#[inline]
fn color_to_lch(color: iced::Color) -> palette::Oklch<f32> {
    let [r, g, b, a] = color.into_linear();

    palette::rgb::Srgba::<f32>::from_linear(palette::LinSrgba::new(r, g, b, a)).into_color()
}

#[inline]
fn lch_to_color(lch: palette::Oklch<f32>) -> iced::Color {
    let color: palette::LinSrgba = lch.into_color();
    let (r, g, b, a) = color.into_components();
    iced::Color::from_linear_rgba(r, g, b, a)
}
