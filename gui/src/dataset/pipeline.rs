//! Data pipeline.

use iced::widget;
use polars::prelude as pl;
use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
    time,
};

use crate::icon;

#[derive(Clone, Debug)]
pub enum Message {
    /// Set the active layer.
    SetActiveLayer(TransformId),
    /// Copy text to the system clipboard.
    CopyToClipboard(String),
    PromptTransformScript,
    PushTransformScript(PathBuf),
    TransformOutputUpdated(TransformId),
    IpcDataframeRequest {
        transform: TransformId,
        tx: crate::data_server::DataRequestTx,
    },
    IpcDataframeProdcued {
        transform: TransformId,
        dataframe: pl::DataFrame,
    },
}

#[derive(derive_more::From)]
pub enum Action {
    None,
    Run(iced::Task<Message>),
    RegisterTransformScript(TransformId),
    UpdateDataframe(pl::DataFrame),
}

pub type TransformId = u8;

#[derive(Clone, Debug)]
pub struct Transform {
    id: TransformId,
    kind: TransformKind,
}

impl Transform {
    pub fn id(&self) -> TransformId {
        self.id
    }

    pub fn kind(&self) -> &TransformKind {
        &self.kind
    }
}

impl<'a> iced::advanced::text::IntoFragment<'a> for &'a Transform {
    fn into_fragment(self) -> widget::text::Fragment<'a> {
        self.kind.into_fragment()
    }
}

#[derive(Clone, Debug)]
pub enum TransformKind {
    Script {
        file: PathBuf,
        runner: TransformScriptRunner,
    },
}

impl<'a> iced::advanced::text::IntoFragment<'a> for &'a TransformKind {
    fn into_fragment(self) -> widget::text::Fragment<'a> {
        match self {
            TransformKind::Script { file, runner: _ } => Cow::Owned(format!(
                "{}",
                file.file_name()
                    .expect("file name should exist")
                    .to_string_lossy()
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TransformScriptRunner {
    cmd: String,
}

#[derive(Clone)]
pub(super) struct Pipeline {
    /// Original dataframe.
    raw: pl::DataFrame,
    transforms: Vec<Transform>,
    cache: HashMap<TransformId, pl::DataFrame>,
    active_layer: TransformId,
}

impl Pipeline {
    pub(super) fn new(df: pl::DataFrame) -> Self {
        Self {
            raw: df,
            transforms: Default::default(),
            cache: Default::default(),
            active_layer: Default::default(),
        }
    }

    fn push(&mut self, transform: TransformKind) -> TransformId {
        let id = if let Some(id) = self.transforms.iter().map(|transform| transform.id).max() {
            id + 1
        } else {
            0
        };

        let transform = Transform {
            id,
            kind: transform,
        };

        self.transforms.push(transform);
        id
    }

    fn get_transform(&self, id: TransformId) -> Option<&Transform> {
        self.transforms.iter().find(|transform| transform.id == id)
    }

    /// Get the index (position) of the transform.
    /// Transforms are 0 indexed.
    fn transform_position(&self, id: TransformId) -> Option<usize> {
        self.transforms
            .iter()
            .position(|transform| transform.id == id)
    }

    /// # Returns
    /// `true` if the transform is the last on the stack.
    fn transform_is_last(&self, id: TransformId) -> Option<bool> {
        self.transforms.last().map(|transform| transform.id == id)
    }
}

impl Pipeline {
    pub(super) fn view(&self, dataset: impl AsRef<Path>) -> iced::Element<'_, Message> {
        let btn_ctrl_add_script = widget::button("Script").on_press(Message::PromptTransformScript);
        let controls_r1 = widget::row![btn_ctrl_add_script];
        let controls = widget::column![controls_r1];

        let raw = widget::button("raw").on_press(Message::SetActiveLayer(0));
        let stages = std::iter::once(raw.into())
            .chain(self.transforms.iter().enumerate().map(|(idx, transform)| {
                Self::view_transform_layer(1 + idx as TransformId, transform, &dataset)
            }))
            .collect::<Vec<iced::Element<'_, Message>>>();
        let pipeline = widget::column(stages);

        widget::column![controls, pipeline].into()
    }

    #[inline]
    fn view_transform_layer(
        id: TransformId,
        transform: &Transform,
        dataset: impl AsRef<Path>,
    ) -> iced::Element<'_, Message> {
        match &transform.kind {
            TransformKind::Script { file, .. } => {
                let btn_main =
                    widget::button(widget::text(transform)).on_press(Message::SetActiveLayer(id));
                let tt_main = widget::tooltip(
                    btn_main,
                    widget::container(widget::text(file.to_string_lossy().to_owned())).style(
                        |theme: &iced::Theme| {
                            widget::container::background(theme.palette().background)
                        },
                    ),
                    widget::tooltip::Position::Top,
                )
                .delay(crate::TOOLTIP_DELAY);

                let key = crate::data_server::TransformUri::key_of(&dataset, transform.id);
                let btn_key =
                    widget::button(icon::copy()).on_press(Message::CopyToClipboard(key.clone()));
                let tt_key =
                    widget::tooltip(btn_key, widget::text(key), widget::tooltip::Position::Top)
                        .delay(crate::TOOLTIP_DELAY);

                widget::row![tt_main, tt_key].into()
            }
        }
    }
}

impl Pipeline {
    pub(super) fn update(&mut self, message: Message) -> Action {
        match message {
            Message::SetActiveLayer(idx) => {
                if self.active_layer == idx {
                    return Action::None;
                }

                todo!("set active layer");
            }
            Message::CopyToClipboard(contents) => Action::Run(iced::clipboard::write(contents)),
            Message::PromptTransformScript => self.prompt_new_transform_script(),
            Message::PushTransformScript(path) => {
                let id = self.push(TransformKind::Script {
                    file: path,
                    runner: TransformScriptRunner {
                        cmd: "python".to_string(),
                    },
                });

                Action::RegisterTransformScript(id)
            }
            Message::TransformOutputUpdated(transform) => self.transform_output_updated(transform),
            Message::IpcDataframeRequest { transform, tx } => {
                self.ipc_dataframe_request(transform, tx)
            }
            Message::IpcDataframeProdcued {
                transform,
                dataframe,
            } => self.ipc_dataframe_produced(transform, dataframe),
        }
    }

    fn prompt_new_transform_script(&mut self) -> Action {
        iced::Task::future(
            rfd::AsyncFileDialog::new()
                .set_title("Select script file")
                .add_filter("Python", &["py"])
                .pick_file(),
        )
        .then(|path| match path {
            Some(fh) => iced::Task::done(Message::PushTransformScript(fh.path().to_path_buf())),
            None => iced::Task::none(),
        })
        .into()
    }

    fn transform_output_updated(&mut self, transform: TransformId) -> Action {
        if self
            .transform_is_last(transform)
            .expect("transform should exist")
        {
            if self.active_layer == transform - 1 {
                self.active_layer = transform;
            }

            let df = self
                .cache
                .get(&transform)
                .expect("latest dataframe should be cached");

            return Action::UpdateDataframe(df.clone());
        }

        todo!("recompute dependents");
    }

    fn ipc_dataframe_request(
        &mut self,
        transform: TransformId,
        tx: crate::data_server::DataRequestTx,
    ) -> Action {
        let mut tx = tx.lock().expect("could not get ipc response channel");
        let tx = tx.take().expect("ipc response channel already taken");

        let idx = self
            .transform_position(transform)
            .expect("transform should exist");
        let df = if idx == 0 {
            self.raw.clone()
        } else if let Some(df) = self.cache.get_mut(&transform) {
            df.clone()
        } else {
            todo!("recalculate dataframe")
        };

        tx.send(df).expect("could not send ipc response");
        Action::None
    }

    fn ipc_dataframe_produced(
        &mut self,
        transform: TransformId,
        dataframe: pl::DataFrame,
    ) -> Action {
        self.cache.insert(transform, dataframe);
        self.transform_output_updated(transform)
    }
}
