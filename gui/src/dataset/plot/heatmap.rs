//! Heatmap.
use crate::dataset::plot::heatmap::data::PlotUpdate;

use super::SharedDataframe;
use iced_aksel as aksel;
use palette::ShiftHue;
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
    show_centers: bool,
}

impl Options {
    pub fn new(index: Index) -> Self {
        Self {
            index,
            show_centers: Default::default(),
        }
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
    DataframeChanged,
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
    df: SharedDataframe,
    options: Options,
}

impl State {
    pub fn new(df: SharedDataframe, options: Options) -> Self {
        let chart = Self::chart(
            &df.read().expect("dataframe should be readable"),
            &options.index,
        );
        let data = data::State::new(df.clone(), options.index.clone(), options.show_centers);

        Self {
            chart,
            data: aksel::Cached::new(data),
            df,
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

        let colorbar = iced::widget::container(self.colorbar()).padding(10.0);
        let plot = iced::widget::row![chart, colorbar];

        let data_controls = self.data.get().view();
        iced::widget::column![plot, data_controls.map(Message::Data)].into()
    }

    fn colorbar(&self) -> iced::Element<'_, Message> {
        let data = self.data.get();
        let df = self.df.read().expect("dataframe shoudl exist");
        let (zmin, zmax) = match &data.index.z.values {
            axis::IndexValues::Index => ("0".to_string(), df.height().to_string()),
            axis::IndexValues::Series(name) => {
                let (zmin, zmax) =
                    super::utils::column_minmax_f64(df.column(name).expect("column should exist"));
                (format!("{zmin:0.2e}"), format!("{zmax:.2e}"))
            }
        };
        let zmin = iced::widget::text(zmin);
        let zmax = iced::widget::text(zmax);

        let colorbar = iced::widget::container(iced::widget::space::vertical().width(20.0)).style(
            |theme: &iced::Theme| {
                let zmin_color = theme.palette().primary;
                let zmax_color = super::utils::lch_to_color(
                    super::utils::color_to_lch(zmin_color.clone()).shift_hue(180.0),
                );
                iced::widget::container::background(iced::Background::Gradient(
                    iced::Gradient::Linear(
                        iced::gradient::Linear::new(0.0)
                            .add_stop(0.0, zmin_color)
                            .add_stop(1.0, zmax_color),
                    ),
                ))
            },
        );
        iced::widget::column![zmax, iced::widget::center_x(colorbar), zmin]
            .width(iced::Shrink)
            .into()
    }
}

impl State {
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::DataframeChanged => {
                match self.data.edit().update(data::Message::DataframeChanged) {
                    data::Action::None => Action::None,
                    data::Action::UpdatePlot(update) => {
                        self.update_plot(update);
                        Action::None
                    }
                }
            }
            Message::Plot(message) => self.update_chart(message),
            Message::Data(message) => match self.data.edit().update(message) {
                data::Action::None => Action::None,
                data::Action::UpdatePlot(update) => {
                    self.update_plot(update);
                    Action::None
                }
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

    fn update_plot(&mut self, update: PlotUpdate) {
        match update {
            data::PlotUpdate::AllChanged => {
                self.rescale_xaxis();
                self.rescale_yaxis();
            }
            data::PlotUpdate::XValuesChanged => {
                self.rescale_xaxis();
            }
            data::PlotUpdate::YValuesChanged => {
                self.rescale_yaxis();
            }
            data::PlotUpdate::ZValuesChanged => {}
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
    use std::collections::BTreeMap;

    use super::super::utils;
    use super::{SharedDataframe, ValueType, axis};
    use iced_aksel::{self as aksel, interaction::IntoArea};
    use palette::{Darken, ShiftHue};

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
        DataframeChanged,
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
        AllChanged,
    }

    #[derive(Debug, Clone, derive_more::From)]
    pub enum Action {
        None,
        UpdatePlot(PlotUpdate),
    }

    pub struct State {
        pub(super) df: SharedDataframe,
        pub(super) index: super::Index,
        /// Show region centers.
        pub(super) show_centers: bool,
        /// Indicates whether multiple values exist for a single coordinate.
        points: super::Points,
        hovered_id: Option<aksel::interaction::Id>,
        selected_id: Option<aksel::interaction::Id>,
    }

    impl State {
        pub fn new(df: SharedDataframe, index: super::Index, show_centers: bool) -> Self {
            let dfg = df.read().expect("dataframe should be readable");
            let points = super::Points::new(dfg.height());
            drop(dfg);
            Self {
                df,
                index,
                show_centers,
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
                Message::DataframeChanged => self.refresh_with_dataframe(),
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

        /// Reset state based on the current dataframe.
        fn refresh_with_dataframe(&mut self) -> Action {
            let df = self.df.read().expect("dataframe should be readable");
            self.points = super::Points::new(df.height());
            self.hovered_id = None;
            self.selected_id = None;

            if let axis::IndexValues::Series(col) = &self.index.x.values {
                if df.column(col).is_err() {
                    self.index.x.values = axis::IndexValues::Index;
                }
            };
            if let axis::IndexValues::Series(col) = &self.index.y.values {
                if df.column(col).is_err() {
                    self.index.y.values = axis::IndexValues::Index;
                }
            };
            if let axis::IndexValues::Series(col) = &self.index.z.values {
                if df.column(col).is_err() {
                    self.index.z.values = axis::IndexValues::Index;
                }
            };

            Action::UpdatePlot(PlotUpdate::AllChanged)
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
        // TODO: Handle degenerate case of 0 or 1 points to plot.
        fn draw(&self, plot: &mut aksel::Plot<ValueType, super::Message>, base_color: iced::Color) {
            #[derive(Clone)]
            struct Coord {
                x: f64,
                y: f64,
            }
            impl PartialEq for Coord {
                fn eq(&self, other: &Self) -> bool {
                    self.x == other.x && self.y == other.y
                }
            }
            impl Eq for Coord {}

            impl PartialOrd for Coord {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    let mut ord = self.x.total_cmp(&other.x);
                    if matches!(ord, std::cmp::Ordering::Equal) {
                        ord = self.y.total_cmp(&other.y);
                    }
                    Some(ord)
                }
            }
            impl Ord for Coord {
                fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                    self.partial_cmp(other).unwrap()
                }
            }

            struct Point {
                coord: Coord,
                z: f64,
            }

            let df = self.df.read().expect("dataframe should be readable");
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
            let z = match &self.index.z.values {
                axis::IndexValues::Series(column) => {
                    let z = df.column(column).unwrap();
                    utils::column_to_values_f64(z)
                }
                axis::IndexValues::Index => {
                    (0..df.height()).map(|z| z as ValueType).collect::<Vec<_>>()
                }
            };
            let zmin = z.iter().fold(f64::INFINITY, |a, b| a.min(*b));
            let zmax = z.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));

            let coords = itertools::izip!(x, y, z)
                .map(|(x, y, z)| Point {
                    coord: Coord { x, y },
                    z,
                })
                .collect::<Vec<_>>();
            let mut pts = BTreeMap::<Coord, Vec<f64>>::new();
            for coord in coords {
                pts.entry(coord.coord)
                    .and_modify(|vals| vals.push(coord.z))
                    .or_insert(vec![coord.z]);
            }

            let xmin = pts
                .keys()
                .fold(f64::INFINITY, |cur, coord| cur.min(coord.x));
            let ymin = pts
                .keys()
                .fold(f64::INFINITY, |cur, coord| cur.min(coord.y));
            let xmax = pts
                .keys()
                .fold(f64::NEG_INFINITY, |cur, coord| cur.max(coord.x));
            let ymax = pts
                .keys()
                .fold(f64::NEG_INFINITY, |cur, coord| cur.max(coord.y));

            let base_color_lch = utils::color_to_lch(base_color);
            let colors = pts
                .iter()
                .map(|(coord, vals)| {
                    let colors = vals
                        .iter()
                        .map(|z| {
                            if z.is_nan() {
                                palette::Oklch::from_components((0.0, 0.0, 0.0))
                            } else {
                                let scale = (z - zmin) / (zmax - zmin);
                                base_color_lch.shift_hue(scale as f32 * 180.0)
                            }
                        })
                        .collect();

                    (coord.clone(), colors)
                })
                .collect::<BTreeMap<Coord, Vec<_>>>();

            let centers = pts
                .keys()
                .map(|coord| (coord.x, coord.y))
                .collect::<Vec<_>>();

            const SHIFT_SCALE: f64 = 1.0;
            let xshift = (xmax - xmin) * SHIFT_SCALE;
            let yshift = (ymax - ymin) * SHIFT_SCALE;
            let xbmin = xmin - xshift;
            let xbmax = xmax + xshift;
            let ybmin = ymin - yshift;
            let ybmax = ymax + yshift;
            let voronoi = voronator::VoronoiDiagram::<voronator::delaunator::Point>::from_tuple(
                &(xbmin, ybmin),
                &(xbmax, ybmax),
                &centers,
            )
            .expect("could not create voronoi diagram");

            voronoi
                .cells()
                .into_iter()
                .filter(|cell| cell.points().len() > 0)
                .enumerate()
                .map(|(idx, cell)| {
                    let pts = cell
                        .points()
                        .iter()
                        .map(|pt| aksel::PlotPoint::new(pt.x, pt.y))
                        .collect();

                    let color = colors.values().skip(idx).take(1).collect::<Vec<_>>()[0][0];
                    let color = utils::lch_to_color(color);
                    aksel::shape::Area::new(pts).fill(color)
                })
                .for_each(|cell| plot.render(cell));

            if self.show_centers {
                pts.iter()
                    .enumerate()
                    .map(|(idx, (coord, vals))| {
                        let color = colors.values().skip(idx).take(1).collect::<Vec<_>>()[0][0];
                        let color = color.darken(0.3);
                        let color = utils::lch_to_color(color);

                        aksel::shape::Ellipse::circle(
                            aksel::PlotPoint::new(coord.x, coord.y),
                            aksel::Measure::Screen(2.0),
                        )
                        .fill(color)
                    })
                    .for_each(|pt| plot.render(pt));
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
            let label = if ctx.tick.level == 0 {
                let text = format!("{:.02e}", ctx.tick.value);
                Some(ctx.label(text))
            } else {
                None
            };

            let mut tickline = ctx.tickline();
            if ctx.tick.level == 0 {
                tickline.length = 10.0.into();
            } else {
                tickline.length = 5.0.into();
            };

            aksel::axis::TickResult {
                label,
                label_badge: Some(ctx.label_badge()),
                tick_line: Some(tickline),
                grid_line: Some(ctx.gridline()),
                label_priority: None,
            }
        }
    }
}
