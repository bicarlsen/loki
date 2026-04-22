//! Heatmap.
use super::SharedDataframe;
use iced_aksel as aksel;
use polars::prelude as pl;

type ValueType = f64;

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

#[derive(Debug, Clone)]
pub struct Options {
    index: Index,
}

impl Options {
    pub fn new(index: Index) -> Self {
        Self { index }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Index {
    x: axis::Axis,
    y: axis::Axis,
    /// Color axis.
    z: axis::Axis,
}

impl Index {
    pub fn new(x: axis::Axis, y: axis::Axis, z: axis::Axis) -> Self {
        Self { x, y, z }
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
    chart: aksel::State<&'static str, ValueType>,
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
        chart.set_axis(
            axis::Y_AXIS_ID,
            index.y.to_aksel(df, aksel::axis::Position::Left),
        );

        chart
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        let chart = aksel::Chart::new(&self.chart)
            .plot_data(self.data.get(), axis::X_AXIS_ID, axis::Y_AXIS_ID)
            .on_drag(|event: aksel::DragEvent<aksel::Delta>| {
                (event.button_held == iced::mouse::Button::Left)
                    .then_some(Message::Plot(PlotInteraction::Dragged(event.delta)))
            })
            .on_scroll(|event| Message::Plot(PlotInteraction::Scrolled(event)));

        let data_controls = self.data.get().view();
        iced::widget::column![
            iced::widget::container(chart),
            data_controls.map(Message::Data)
        ]
        .into()
    }
}

impl State {
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
                    data::PlotUpdate::ZValuesChanged => Action::None,
                },
            },
        }
    }

    fn update_chart(&mut self, message: PlotInteraction) -> Action {
        match message {
            PlotInteraction::Dragged(delta) => {
                // TODO: Account for multiple y axes
                self.chart
                    .pan_axes(axis::X_AXIS_ID, axis::Y_AXIS_ID, delta.x, delta.y);
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
                    .axis_mut(&axis::Y_AXIS_ID)
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
        let axis = data.index.y.to_aksel(
            &data.df.read().expect("dataframe should be readable"),
            aksel::axis::Position::Left,
        );
        self.chart.set_axis(axis::Y_AXIS_ID, axis);
    }
}

mod data {
    use super::super::utils;
    use super::{SharedDataframe, ValueType, axis};
    use iced_aksel::{self as aksel, interaction::IntoArea};
    use palette::ShiftHue;

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

    #[derive(Debug, Clone)]
    pub enum Message {
        UpdateXValues(axis::IndexValues),
        UpdateYValues(axis::IndexValues),
        UpdateZValues(axis::IndexValues),
        // Plot(PlotInteraction),
    }

    #[derive(Debug, Clone)]
    pub enum PlotUpdate {
        XValuesChanged,
        YValuesChanged,
        ZValuesChanged,
    }

    #[derive(Debug, Clone, derive_more::From)]
    pub enum Action {
        None,
        UpdatePlot(PlotUpdate),
    }

    pub struct State {
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
    }

    impl State {
        pub fn view(&self) -> iced::Element<'_, Message> {
            let pl_xaxis = self.pl_index_axis(&self.index.x.values, Message::UpdateXValues);
            let pl_xaxis = iced::widget::row![iced::widget::text("x-axis"), pl_xaxis];
            let pl_yaxis = self.pl_index_axis(&self.index.y.values, Message::UpdateYValues);
            let pl_yaxis = iced::widget::row![iced::widget::text("y-axis"), pl_yaxis];
            let xy_axes = iced::widget::row![pl_xaxis, pl_yaxis];

            let pl_z = self.pl_index_axis(&self.index.z.values, Message::UpdateZValues);
            let pl_z = iced::widget::row![iced::widget::text("z"), pl_z];

            iced::widget::column![xy_axes, pl_z].into()
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
    }

    impl State {
        pub fn update(&mut self, message: Message) -> Action {
            match message {
                Message::UpdateXValues(value) => self.update_x_axis_values(value),
                Message::UpdateYValues(value) => self.update_y_axis_values(value),
                Message::UpdateZValues(value) => self.update_z_axis_values(value),
                // Message::Plot(message) => {
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

        fn update_y_axis_values(&mut self, values: axis::IndexValues) -> Action {
            // TODO: account for logarithmic scale
            self.index.y.values = values;
            PlotUpdate::YValuesChanged.into()
        }

        fn update_z_axis_values(&mut self, values: axis::IndexValues) -> Action {
            // TODO: account for logarithmic scale
            self.index.z.values = values;
            PlotUpdate::ZValuesChanged.into()
        }
    }

    impl State {
        fn draw(&self, plot: &mut aksel::Plot<ValueType, super::Message>, base_color: iced::Color) {
            let df = self.df.read().expect("dataframe should be readable");
            let base_color_lch = utils::color_to_lch(base_color);

            let colors = match &self.index.z.values {
                axis::IndexValues::Index => {
                    let height = df.height();
                    (0..height)
                        .map(|idx| idx as f32 / height as f32)
                        .map(|scale| base_color_lch.shift_hue(scale * 180.0))
                        .map(utils::lch_to_color)
                        .collect::<Vec<_>>()
                }
                axis::IndexValues::Series(column) => {
                    let col = df.column(column).expect("column should exist");
                    let (min, max) = utils::column_minmax_f64(col);
                    let scaled = (col - min) / max;
                    utils::column_to_values_f64(&scaled)
                        .into_iter()
                        .map(|scale| base_color_lch.shift_hue(scale as f32 * 180.0))
                        .map(utils::lch_to_color)
                        .collect::<Vec<_>>()
                }
            };

            // TODO: Handle hovered region color.

            let x = match &self.index.x.values {
                axis::IndexValues::Series(column) => {
                    let x = df.column(column).unwrap();
                    utils::column_to_values_f64(x)
                }
                axis::IndexValues::Index => {
                    (0..df.height()).map(|x| x as ValueType).collect::<Vec<_>>()
                }
            };
            let y = match &self.index.y.values {
                axis::IndexValues::Series(column) => {
                    let y = df.column(column).unwrap();
                    utils::column_to_values_f64(y)
                }
                axis::IndexValues::Index => {
                    (0..df.height()).map(|y| y as ValueType).collect::<Vec<_>>()
                }
            };

            let points = itertools::izip!(self.points.iter_idx(), x, y, colors);
            for (iid, x, y, color) in points {
                Self::draw_point(plot, iid, x, y, color);
            }
        }

        #[inline]
        fn draw_point(
            plot: &mut aksel::Plot<ValueType, super::Message>,
            id: &aksel::interaction::Id,
            x: ValueType,
            y: ValueType,
            color: iced::Color,
        ) {
            // TODO: Use vornoi cover or calculate pixel size
            let marker_size = aksel::Measure::Screen(10.0);
            let shape = aksel::shape::Rectangle::centered(
                aksel::PlotPoint::new(x, y),
                marker_size.clone(),
                marker_size.clone(),
            )
            .fill(color);

            let area = shape.resolve_area(&plot);
            plot.render(shape);

            let interaction = aksel::Interaction::new(area)
                .on_enter(|point, event| {
                    super::Message::Plot(super::PlotInteraction::ShapeEnter { point, event })
                })
                .on_exit(|_, _| super::Message::Plot(super::PlotInteraction::ShapeExit));
            // .on_press(|point, event| {
            //     Message::Chart(ChartMessage::PointMouseDown { point, event })
            // })
            plot.push_interaction(id.clone(), interaction);
        }
    }

    impl aksel::PlotData<ValueType, super::Message> for State {
        fn draw(
            &self,
            plot: &mut aksel::Plot<ValueType, super::Message>,
            theme: &iced::advanced::graphics::core::Theme,
        ) {
            // TODO: Allow color control from UI
            let base_color = theme.palette().primary;
            self.draw(plot, base_color)
        }
    }
}

pub mod axis {
    pub use super::super::axis::{AxisScale, IndexValues};
    use super::super::utils;
    use super::ValueType;
    use iced_aksel as aksel;
    use polars::prelude as pl;

    pub(super) const X_AXIS_ID: &str = "x";
    pub(super) const Y_AXIS_ID: &str = "y";

    #[derive(Default, Debug, Clone)]
    pub struct Axis {
        pub(super) scale: AxisScale,
        pub(super) values: IndexValues,
    }

    impl Axis {
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
        ) -> aksel::Axis<ValueType> {
            match &self.values {
                super::axis::IndexValues::Index => aksel::Axis::new(
                    aksel::scale::Linear::new(0.0, df.height() as ValueType),
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

    fn axis_renderer_ticks()
    -> impl Fn(aksel::axis::TickContext<ValueType>) -> aksel::axis::TickResult + 'static {
        move |ctx: aksel::axis::TickContext<ValueType>| {
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
