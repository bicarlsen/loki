//! Scatter plot.

use super::{SharedDataframe, chart, utils};
use iced_aksel::{self as aksel};
use polars::prelude as pl;

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

    /// Get a point's id by index.
    pub fn get_by_idx(&self, idx: usize) -> Option<aksel::interaction::Id> {
        self.by_idx.get(idx).cloned()
    }

    /// Get a point's index by id.
    pub fn get_by_id(&self, id: &aksel::interaction::Id) -> Option<usize> {
        self.by_id.get(id).cloned()
    }

    /// Iterate over the points by index.
    pub fn iter_idx(&self) -> impl Iterator<Item = &aksel::interaction::Id> {
        self.by_idx.iter()
    }

    pub fn clear(&mut self) {
        self.by_id.clear();
        self.by_idx.clear();
    }
}

#[derive(Clone, Debug, Default)]
pub struct Index {
    x: axis::IndexAxis,
    y: Vec<axis::ValueAxis>,
}

impl Index {
    pub fn new(x: axis::IndexAxis, y: Vec<axis::ValueAxis>) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    index: Index,
}

impl Options {
    pub fn new(index: Index) -> Self {
        Self { index }
    }
}

#[derive(Debug, Clone)]
pub enum PlotInteraction {
    Dragged(aksel::Delta),
    Scrolled(aksel::ScrollEvent<iced::Point>),
    ShapeEnter {
        point: aksel::interaction::Id,
        event: aksel::EnterEvent,
    },
    ShapeExit,
}

#[derive(Debug, Clone, derive_more::From)]
pub enum Message {
    Plot(PlotInteraction),
    Data(data::Message),
}

pub enum Action {
    None,
    DataHovered(Option<usize>),
}

pub(super) struct State {
    chart: aksel::State<&'static str, chart::ValueType>,
    data: aksel::Cached<data::State>,
    options: Options,
}

impl State {
    pub fn new(df: SharedDataframe, options: Options) -> Self {
        let chart = Self::chart(
            &df.read().expect("dataframe should be readable"),
            &options.index,
        );
        let data = data::State::new(df, options.index.clone());

        Self {
            chart,
            data: aksel::Cached::new(data),
            options,
        }
    }

    #[inline]
    fn chart(df: &pl::DataFrame, index: &Index) -> aksel::State<&'static str, f64> {
        let mut chart = aksel::State::new();
        chart.set_axis(
            axis::X_AXIS_ID,
            index.x.to_aksel(df, aksel::axis::Position::Bottom),
        );

        for axis in index.y.iter() {
            let id = axis.aksel_id();
            let axis = axis.to_aksel(&df);
            chart.set_axis(id, axis);
        }

        chart
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        let chart = aksel::Chart::new(&self.chart)
            .plot_data(self.data.get(), axis::X_AXIS_ID, axis::Y_AXIS_IDS[0])
            .on_drag(|event: aksel::DragEvent<aksel::Delta>| {
                (event.button_held == iced::mouse::Button::Left)
                    .then_some(Message::Plot(PlotInteraction::Dragged(event.delta)))
            })
            .on_scroll(|event| Message::Plot(PlotInteraction::Scrolled(event)));
        // TODO: Option to show markers
        // .marker(
        //         &X_AXIS_ID,
        //         aksel::axis::MarkerPosition::Cursor,
        //         axis_renderer_marker,
        //     )
        //     .marker(
        //         &Y_AXIS_IDS[0],
        //         aksel::axis::MarkerPosition::Cursor,
        //         axis_renderer_marker,
        //     )

        let data_controls = self.data.get().view();
        iced::widget::column![
            iced::widget::container(chart),
            data_controls.map(Message::Data)
        ]
        .into()
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Plot(message) => self.update_chart(message),
            Message::Data(message) => match self.data.edit().update(message) {
                data::Action::None => Action::None,
                data::Action::UpdatePlot(update) => match update {
                    data::PlotUpdate::XValuesChanged => {
                        self.rescale_xaxis();
                        Action::None
                    }
                    data::PlotUpdate::YValuesChanged => {
                        self.rescale_yaxis();
                        Action::None
                    }
                },
            },
        }
    }

    fn update_chart(&mut self, message: PlotInteraction) -> Action {
        match message {
            PlotInteraction::Dragged(delta) => {
                // TODO: Account for multiple y axes
                self.chart
                    .pan_axes(axis::X_AXIS_ID, axis::Y_AXIS_IDS[0], delta.x, delta.y);
                Action::None
            }
            PlotInteraction::Scrolled(aksel::ScrollEvent {
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
                    .axis_mut(&axis::X_AXIS_ID)
                    .zoom(zoom_factor, Some(position.x));
                self.chart
                    .axis_mut(&axis::Y_AXIS_IDS[0])
                    .zoom(zoom_factor, Some(position.y));

                Action::None
            }
            PlotInteraction::ShapeEnter { point, event } => {
                let idx = self
                    .data
                    .get()
                    .points()
                    .get_by_id(&point)
                    .expect("point should exist");
                self.data
                    .edit()
                    .update_plot_interaction(data::PlotInteraction::ShapeEnter { point, event });

                Action::DataHovered(Some(idx))
            }
            PlotInteraction::ShapeExit => {
                self.data
                    .edit()
                    .update_plot_interaction(data::PlotInteraction::ShapeExit);

                Action::DataHovered(None)
            }
        }
    }

    fn rescale_xaxis(&mut self) {
        let data = self.data.get();
        let axis = data.index.x.to_aksel(
            &data.df.read().expect("dataframe should be readable"),
            aksel::axis::Position::Bottom,
        );
        self.chart.set_axis(axis::X_AXIS_ID, axis);
    }

    fn rescale_yaxis(&mut self) {
        let data = self.data.get();
        let axis = data.index.y[0].to_aksel(&data.df.read().expect("dataframe should be readable"));
        self.chart.set_axis(axis::Y_AXIS_IDS[0], axis);
    }
}

mod data {
    use crate::dataset::SharedDataframe;

    use super::super::{chart, utils};
    use super::{axis, trace};
    use iced_aksel::{self as aksel, interaction::IntoArea};
    use palette::ShiftHue;
    use polars::prelude as pl;

    #[derive(Debug, Clone)]
    pub enum PlotInteraction {
        ShapeEnter {
            point: aksel::interaction::Id,
            event: aksel::EnterEvent,
        },
        ShapeExit,
        // PointMouseDown {
        //     point: aksel::interaction::Id,
        //     event: aksel::PressEvent<iced::Point>,
        // },
    }

    #[derive(Debug, Clone, derive_more::From)]
    pub enum Message {
        UpdateXValues(axis::IndexValues),
        TraceGroup {
            axis: axis::ValueAxisId,
            message: trace::GroupMessage,
        },
        // Plot(PlotInteraction),
    }

    #[derive(Debug, Clone)]
    pub enum PlotUpdate {
        XValuesChanged,
        YValuesChanged,
    }

    #[derive(Debug, Clone, derive_more::From)]
    pub enum Action {
        None,
        UpdatePlot(PlotUpdate),
    }

    pub(super) struct State {
        pub(super) df: SharedDataframe,
        pub(super) index: super::Index,
        points: super::Points,
        hovered_id: Option<aksel::interaction::Id>,
        selected_id: Option<aksel::interaction::Id>,
    }

    impl State {
        pub fn new(df: SharedDataframe, index: super::Index) -> Self {
            let points =
                super::Points::new(df.read().expect("dataframe should be readable").height());
            Self {
                df,
                index,
                points,
                hovered_id: None,
                selected_id: None,
            }
        }

        pub fn points(&self) -> &super::Points {
            &self.points
        }

        pub fn view(&self) -> iced::Element<'_, Message> {
            let pl_xaxis = self.pl_index_axis(&self.index.x.values, Message::UpdateXValues);
            let pl_xaxis = iced::widget::row![iced::widget::text("x-axis"), pl_xaxis];

            let controls = self
                .index
                .y
                .iter()
                .map(|axis| self.pl_values_axis(axis))
                .collect::<Vec<_>>();
            let yaxis = iced::widget::column(controls);

            iced::widget::column![pl_xaxis, yaxis].into()
        }

        fn pl_index_axis<'a, F>(
            &'a self,
            selected: &'a axis::IndexValues,
            message: F,
        ) -> iced::Element<'a, Message>
        where
            F: Fn(axis::IndexValues) -> Message + 'a,
        {
            let columns = self
                .df
                .read()
                .expect("dataframe should be readable")
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
                    axis::IndexValues::Index => None,
                    axis::IndexValues::Series(column) => Some(column.clone()),
                },
                move |selection| {
                    let values = if selection.is_empty() {
                        axis::IndexValues::Index
                    } else {
                        axis::IndexValues::Series(selection)
                    };

                    message(values)
                },
            )
            .placeholder("<index>")
            .into()
        }

        fn pl_values_axis<'a>(&'a self, axis: &'a axis::ValueAxis) -> iced::Element<'a, Message> {
            let columns = self
                .df
                .read()
                .expect("dataframe should be readable")
                .schema()
                .iter()
                .map(|(name, _)| name.to_string())
                .collect::<Vec<_>>();

            let axis_id = axis.id();
            axis.traces().view(columns.clone()).map(move |message| {
                Message::TraceGroup {
                    axis: axis_id,
                    message,
                }
                .into()
            })
        }
    }

    impl State {
        pub fn update(&mut self, message: Message) -> Action {
            match message {
                Message::UpdateXValues(value) => self.update_x_axis_values(value),
                Message::TraceGroup { axis, message } => {
                    match self.index.y[axis as usize].traces_mut().update(message) {
                        trace::GroupAction::None => Action::None,
                        trace::GroupAction::TracesUpdated => {
                            Action::UpdatePlot(PlotUpdate::YValuesChanged)
                        }
                    }
                } // Message::Plot(message) => {
                  //     self.update_plot_interaction(message);
                  //     Action::None
                  // }
            }
        }

        pub fn update_plot_interaction(&mut self, message: PlotInteraction) {
            match message {
                PlotInteraction::ShapeEnter { point, event: _ } => {
                    let _ = self.hovered_id.insert(point);
                }
                PlotInteraction::ShapeExit => {
                    let _ = self.hovered_id.take();
                }
            }
        }

        fn update_x_axis_values(&mut self, values: axis::IndexValues) -> Action {
            // TODO: account for logarithmic scale
            self.index.x.values = values;
            PlotUpdate::XValuesChanged.into()
        }
    }

    impl State {
        fn draw_axis(
            &self,
            plot: &mut aksel::Plot<chart::ValueType, super::Message>,
            base_color: iced::Color,
            axis: &axis::ValueAxis,
        ) {
            let num_traces = axis.traces().len();
            assert_ne!(num_traces, 0, "axis traces must not be empty");

            let df = self.df.read().expect("dataframe shoudl be readable");
            let base_color_lch = utils::color_to_lch(base_color);
            let trace_color_shift = 180.0 / num_traces as f32;
            for (idx, trace) in axis.traces().iter().enumerate() {
                let trace_color = base_color_lch
                    .clone()
                    .shift_hue(trace_color_shift * idx as f32);

                let mut colors = match trace.color() {
                    trace::Color::Default => {
                        let color = utils::lch_to_color(trace_color);
                        vec![color; df.height()]
                    }
                    trace::Color::Column(column) => {
                        let colors = df.column(&column).expect("color column should exist");
                        let (min, max) = utils::column_minmax_f64(colors);
                        let colors = utils::column_to_values_f64(colors);

                        let range = max - min;
                        if approx::abs_diff_eq!(range, 0.0) {
                            let color = utils::lch_to_color(trace_color);
                            vec![color; df.height()]
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
                                .map(utils::lch_to_color)
                                .collect::<Vec<_>>()
                        }
                    }
                    trace::Color::Custom(_) => todo!(),
                };

                if let Some(id) = &self.hovered_id {
                    let idx = self.points.get_by_id(id).expect("point should exist");
                    let color = colors.get_mut(idx).expect("color should exist");
                    let lch = utils::color_to_lch(color.clone());
                    let lch = lch.shift_hue(120.0);
                    *color = utils::lch_to_color(lch);
                }

                self.draw_trace(plot, colors, trace);
            }
        }

        fn draw_trace(
            &self,
            plot: &mut aksel::Plot<chart::ValueType, super::Message>,
            colors: Vec<iced::Color>,
            trace: &trace::Trace,
        ) {
            let df = self.df.read().expect("dataframe should be readable");
            let column_label = trace.column();
            let y = df.column(column_label).unwrap();
            let y = super::utils::column_to_values_f64(y);

            let x = match &self.index.x.values {
                axis::IndexValues::Series(column) => {
                    let x = df.column(&column).unwrap();
                    super::utils::column_to_values_f64(x)
                }
                axis::IndexValues::Index => (0..df.height())
                    .map(|x| x as chart::ValueType)
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
            plot: &mut aksel::Plot<chart::ValueType, super::Message>,
            marker: trace::Marker,
            id: &aksel::interaction::Id,
            x: chart::ValueType,
            y: chart::ValueType,
            color: iced::Color,
            marker_size: trace::MarkerSize,
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
                        .on_enter(|point, event| {
                            super::Message::Plot(super::PlotInteraction::ShapeEnter {
                                point,
                                event,
                            })
                        })
                        .on_exit(|_, _| super::Message::Plot(super::PlotInteraction::ShapeExit))
                    // .on_press(|point, event| {
                    //     Message::Chart(ChartMessage::PointMouseDown { point, event })
                    // })
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
                        .on_enter(|point, event| {
                            super::Message::Plot(super::PlotInteraction::ShapeEnter {
                                point,
                                event,
                            })
                        })
                        .on_exit(|_, _| super::Message::Plot(super::PlotInteraction::ShapeExit))
                    // .on_press(|point, event| {
                    //     Message::Chart(ChartMessage::PointMouseDown { point, event })
                    // })
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
                        .on_enter(|point, event| {
                            super::Message::Plot(super::PlotInteraction::ShapeEnter {
                                point,
                                event,
                            })
                        })
                        .on_exit(|_, _| super::Message::Plot(super::PlotInteraction::ShapeExit))
                    // .on_press(|point, event| {
                    //     Message::Chart(ChartMessage::PointMouseDown { point, event })
                    // })
                }
            };

            plot.push_interaction(id.clone(), interaction);
        }
    }

    impl aksel::PlotData<chart::ValueType, super::Message> for State {
        fn draw(
            &self,
            plot: &mut aksel::Plot<chart::ValueType, super::Message>,
            theme: &iced::advanced::graphics::core::Theme,
        ) {
            for axis in self.index.y.iter() {
                let base_color = theme.palette().primary;
                self.draw_axis(plot, base_color, axis);
            }
        }
    }
}

pub mod axis {
    pub use super::super::axis::{AxisScale, IndexValues};
    use super::super::{chart, utils};
    use super::trace;
    use iced_aksel as aksel;
    use polars::prelude as pl;

    pub type ValueAxisId = u8;

    pub const X_AXIS_ID: &str = "x";
    pub const Y_AXIS_IDS: [&str; 1] = ["y0"];

    #[derive(Default, Debug, Clone)]
    pub struct IndexAxis {
        pub(super) scale: AxisScale,
        pub(super) values: IndexValues,
    }

    impl IndexAxis {
        pub fn new(values: IndexValues) -> Self {
            Self {
                scale: AxisScale::Linear,
                values,
            }
        }

        pub fn to_aksel(
            &self,
            df: &pl::DataFrame,
            position: aksel::axis::Position,
        ) -> aksel::Axis<chart::ValueType> {
            match &self.values {
                IndexValues::Index => aksel::Axis::new(
                    aksel::scale::Linear::new(0.0, df.height() as chart::ValueType),
                    position,
                )
                .with_tick_renderer(axis_renderer_ticks()),
                IndexValues::Series(name) => {
                    let column = df.column(name).unwrap();
                    let (min, max) = utils::column_minmax_f64(column);
                    match self.scale {
                        AxisScale::Linear => {
                            aksel::Axis::new(aksel::scale::Linear::new(min, max), position)
                                .with_tick_renderer(axis_renderer_ticks())
                        }
                        AxisScale::Log => {
                            aksel::Axis::new(aksel::scale::Linear::new(min, max), position)
                                .with_tick_renderer(axis_renderer_ticks())
                        }
                    }
                }
            }
        }
    }

    impl From<ValueAxis> for IndexAxis {
        fn from(value: ValueAxis) -> Self {
            let ValueAxis {
                id,
                scale,
                position,
                traces,
            } = value;
            assert_ne!(traces.len(), 0, "trace group should not be empty");
            let col = traces[0].column().clone();

            Self {
                scale,
                values: IndexValues::Series(col),
            }
        }
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

        pub fn id(&self) -> ValueAxisId {
            self.id
        }

        pub fn traces(&self) -> &trace::TraceGroup {
            &self.traces
        }

        pub fn traces_mut(&mut self) -> &mut trace::TraceGroup {
            &mut self.traces
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

        #[inline]
        pub fn to_aksel(&self, df: &pl::DataFrame) -> aksel::Axis<chart::ValueType> {
            let (min, max) = self.traces.minmax_f64(df);
            match self.scale {
                AxisScale::Linear => {
                    aksel::Axis::new(aksel::scale::Linear::new(min, max), self.position)
                        .with_tick_renderer(axis_renderer_ticks())
                }

                AxisScale::Log => {
                    aksel::Axis::new(aksel::scale::Linear::new(min, max), self.position)
                        .with_tick_renderer(axis_renderer_ticks())
                }
            }
        }
    }

    #[derive(Debug)]
    pub struct IndexConversionError;
    impl TryFrom<IndexAxis> for ValueAxis {
        type Error = IndexConversionError;
        fn try_from(value: IndexAxis) -> Result<Self, Self::Error> {
            let IndexAxis { scale, values } = value;
            let trace = match values {
                IndexValues::Index => return Err(IndexConversionError),
                IndexValues::Series(col) => trace::Trace::new(0, col),
            };
            let traces = trace::TraceGroup {
                traces: vec![trace],
            };

            Ok(Self {
                id: 0,
                scale,
                position: aksel::axis::Position::Left,
                traces,
            })
        }
    }

    fn axis_renderer_marker(
        ctx: aksel::axis::MarkerContext<chart::ValueType>,
    ) -> Option<aksel::axis::Marker> {
        if !ctx.cursor_on_plot && !ctx.cursor_on_axis {
            return None;
        }

        Some(ctx.marker(format!("{:.02e}", ctx.value)))
    }

    fn axis_renderer_ticks()
    -> impl Fn(aksel::axis::TickContext<chart::ValueType>) -> aksel::axis::TickResult + 'static
    {
        move |ctx: aksel::axis::TickContext<chart::ValueType>| {
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
}

mod trace {
    use super::super::utils;
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
        AddTrace,
    }

    pub enum GroupAction {
        None,
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

        pub fn update(&mut self, message: GroupMessage) -> GroupAction {
            match message {
                GroupMessage::UpdateTrace { trace, message } => {
                    match self.traces[trace as usize].update(message) {
                        TraceAction::None => GroupAction::None,
                        TraceAction::ColumnChange => GroupAction::TracesUpdated,
                        TraceAction::Remove => {
                            assert!(self.traces.len() > 1, "last trace should not be removable");
                            self.traces.retain(|t| t.id != trace);
                            GroupAction::TracesUpdated
                        }
                    }
                }
                GroupMessage::AddTrace => {
                    let column = self
                        .traces
                        .last()
                        .expect("trace group can not be empty")
                        .column
                        .clone();
                    self.add_trace(column);

                    GroupAction::TracesUpdated
                }
            }
        }

        pub fn get_trace(&self, id: TraceId) -> Option<&Trace> {
            self.traces.iter().find(|trace| trace.id == id)
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

        pub fn minmax_f64(&self, df: &pl::DataFrame) -> (f64, f64) {
            let mut min = f64::MAX;
            let mut max = f64::MIN;
            for trace in self.traces.iter() {
                let (tmin, tmax) = trace.minmax_f64(df);
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

    pub enum TraceAction {
        None,
        Remove,
        ColumnChange,
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

        pub fn minmax_f64(&self, df: &pl::DataFrame) -> (f64, f64) {
            let mut min = f64::MAX;
            let mut max = f64::MIN;
            let column = df.column(&self.column).unwrap();
            let (cmin, cmax) = utils::column_minmax_f64(column);
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
        pub fn update(&mut self, message: TraceMessage) -> TraceAction {
            match message {
                TraceMessage::ColumnChange(column) => {
                    self.column = column;
                    TraceAction::ColumnChange
                }
                TraceMessage::ColorChange(color) => {
                    self.color = color;
                    TraceAction::None
                }
                TraceMessage::MarkerSizeChange(size) => {
                    self.marker_size = size;
                    TraceAction::None
                }
                TraceMessage::MarkerSizeIncrement => {
                    self.marker_size += 1.0;
                    TraceAction::None
                }
                TraceMessage::MarkerSizeDecrement => {
                    if self.marker_size > 1.0 {
                        self.marker_size -= 1.0;
                    }
                    TraceAction::None
                }
                TraceMessage::Remove => TraceAction::Remove,
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
