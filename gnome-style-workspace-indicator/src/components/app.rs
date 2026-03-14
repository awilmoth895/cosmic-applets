// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use cctk::{
    sctk::reexports::{
        calloop::channel::SyncSender,
        protocols::ext::workspace::v1::client::ext_workspace_handle_v1::{
            self, ExtWorkspaceHandleV1,
        },
    },
    workspace::Workspace,
};
use cosmic::{
    Element, Task, Theme, app,
    applet::cosmic_panel_config::PanelAnchor,
    iced::{
        Alignment,
        Color,
        Event::Mouse,
        Length, Limits, Subscription, event,
        mouse::{self, ScrollDelta},
        widget::{button, column, row},
    },
    iced_core::{Background, Border},
    scroll::DiscreteScrollState,
    surface,
    widget::{Id, autosize, container, horizontal_space, vertical_space},
};

use crate::{
    config,
    wayland::WorkspaceEvent,
    wayland_subscription::{WorkspacesUpdate, workspaces},
};

use std::{process::Command as ShellCommand, sync::LazyLock, time::Duration};

static AUTOSIZE_MAIN_ID: LazyLock<Id> = LazyLock::new(|| Id::new("autosize-main"));

const SCROLL_RATE_LIMIT: Duration = Duration::from_millis(200);

pub fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<IcedWorkspacesApplet>(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Row,
    Column,
}

struct IcedWorkspacesApplet {
    core: cosmic::app::Core,
    workspaces: Vec<Workspace>,
    workspace_tx: Option<SyncSender<WorkspaceEvent>>,
    layout: Layout,
    scroll: DiscreteScrollState,
    hover: bool,
}

impl IcedWorkspacesApplet {
    /// returns the index of the workspace button after which which must be moved to a popup
    /// if it exists.
    fn popup_index(&self) -> Option<usize> {
        let mut index = None;
        let Some(max_major_axis_len) = self.core.applet.suggested_bounds.as_ref().map(|c| {
            // if we have a configure for width and height, we're in a overflow popup
            match self.core.applet.anchor {
                PanelAnchor::Top | PanelAnchor::Bottom => c.width as u32,
                PanelAnchor::Left | PanelAnchor::Right => c.height as u32,
            }
        }) else {
            return index;
        };
        let button_total_size = self.core.applet.suggested_size(true).0
            + self.core.applet.suggested_padding(true).1 * 2
            + 4;
        let btn_count = max_major_axis_len / button_total_size as u32;
        if btn_count >= self.workspaces.len() as u32 {
            index = None;
        } else {
            index = Some((btn_count as usize).min(self.workspaces.len()));
        }
        index
    }
}

#[derive(Debug, Clone)]
enum Message {
    WorkspaceUpdate(WorkspacesUpdate),
    WorkspacePressed(ExtWorkspaceHandleV1),
    WheelScrolled(ScrollDelta),
    WorkspaceOverview,
    Surface(surface::Action),
    HoverEnter,
    HoverExit,
    ContainerClick,
}

impl cosmic::Application for IcedWorkspacesApplet {
    type Message = Message;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = config::APP_ID;

    fn init(core: cosmic::app::Core, _flags: Self::Flags) -> (Self, app::Task<Self::Message>) {
        (
            Self {
                layout: match &core.applet.anchor {
                    PanelAnchor::Left | PanelAnchor::Right => Layout::Column,
                    PanelAnchor::Top | PanelAnchor::Bottom => Layout::Row,
                },
                core,
                workspaces: Vec::new(),
                workspace_tx: Option::default(),
                scroll: DiscreteScrollState::default().rate_limit(Some(SCROLL_RATE_LIMIT)),
                hover: false,
            },
            Task::none(),
        )
    }

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn update(&mut self, message: Self::Message) -> app::Task<Self::Message> {
        match message {
            Message::WorkspaceUpdate(msg) => match msg {
                WorkspacesUpdate::Workspaces(mut list) => {
                    list.retain(|w| !w.state.contains(ext_workspace_handle_v1::State::Hidden));
                    list.sort_by(|w1, w2| w1.coordinates.cmp(&w2.coordinates));
                    self.workspaces = list;
                }
                WorkspacesUpdate::Started(tx) => {
                    self.workspace_tx.replace(tx);
                }
                WorkspacesUpdate::Errored => {
                    // TODO
                }
            },
            Message::WorkspacePressed(id) => {
                if let Some(tx) = self.workspace_tx.as_mut() {
                    let _ = tx.try_send(WorkspaceEvent::Activate(id));
                }
            }
            Message::WheelScrolled(delta) => {
                let discrete_delta = self.scroll.update(delta);
                if discrete_delta.y != 0 {
                    if let Some(w_i) = self
                        .workspaces
                        .iter()
                        .position(|w| w.state.contains(ext_workspace_handle_v1::State::Active))
                    {
                        let d_i = (w_i as isize - discrete_delta.y)
                            .rem_euclid(self.workspaces.len() as isize)
                            as usize;

                        if let Some(tx) = self.workspace_tx.as_mut() {
                            let _ = tx.try_send(WorkspaceEvent::Activate(
                                self.workspaces[d_i].handle.clone(),
                            ));
                        }
                    }
                }
            }
            Message::WorkspaceOverview => {
                let _ = ShellCommand::new("cosmic-workspaces").spawn();
            }
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(a),
                ));
            }
            Message::HoverEnter => {
                self.hover = true;
            }
            Message::HoverExit => {
                self.hover = false;
            }
            Message::ContainerClick => {
                let _ = ShellCommand::new("cosmic-workspaces").spawn();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        if self.workspaces.is_empty() {
            return row![].padding(8).into();
        }
        let horizontal = matches!(
            self.core.applet.anchor,
            PanelAnchor::Top | PanelAnchor::Bottom
        );
        let suggested_total = self.core.applet.suggested_size(true).0
            + self.core.applet.suggested_padding(true).1 * 2;
        let suggested_window_size = self.core.applet.suggested_window_size();
        let popup_index = self.popup_index().unwrap_or(self.workspaces.len());

        let buttons = self.workspaces[..popup_index].iter().map(|w| {
            let content = self.core.applet.text("").font(cosmic::font::bold());

            let (mut width, mut height) = if self.core.applet.is_horizontal() {
                (suggested_total as f32, suggested_window_size.1.get() as f32)
            } else {
                (suggested_window_size.0.get() as f32, suggested_total as f32)
            };

            // height = 2.5;

            // if w.state.contains(ext_workspace_handle_v1::State::Active) {
            //   width = 40.0;
            // } else {
            //     width = 10.0;
            // }
            let width = if w.state.contains(ext_workspace_handle_v1::State::Active) {
                if self.core.applet.is_horizontal() {
                    35.0
                } else {
                    9.5
                }
            } else {
                7.0
            };
            let height = if w.state.contains(ext_workspace_handle_v1::State::Active) {
                if self.core.applet.is_horizontal() {
                    9.5
                } else {
                    35.0
                }
            } else {
                7.0
            };

            // let width = size;
            // let height = 5.0;


            // let content = row!(content, vertical_space().height(Length::Fixed(height)))
            //     .align_y(Alignment::Center);

            // let content = column!(content, horizontal_space().width(Length::Fixed(width)))
            //     .align_x(Alignment::Center);

            let btn = button(
                container(content)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
            )
            .width(Length::Fixed(width))
            .height(Length::Fixed(height))
            .padding(if horizontal {
                // [0,0]
                [0, self.core.applet.suggested_padding(true).1]
            } else {
                // [0,0]
                [self.core.applet.suggested_padding(true).1, 0]
            })
            .on_press(
                if w.state.contains(ext_workspace_handle_v1::State::Active) {
                    Message::WorkspaceOverview
                } else {
                    Message::WorkspacePressed(w.handle.clone())
                },
            )
            .padding(0.0)
            .clip(true);

            btn.class(
                if w.state.contains(ext_workspace_handle_v1::State::Active) {
                    let appearance = |theme: &Theme| {
                        let cosmic = theme.cosmic();
                        button::Style {
                            border: Border {
                                radius: 20.0.into(),
                                ..Default::default()
                            },
                            
                            background: Some(Background::Color(
                                Color {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 1.0,
                                }
                            )),
                            ..button::Style::default()
                        }
                    };
                    cosmic::theme::iced::Button::Custom(Box::new(
                        move |theme, status| match status {
                            button::Status::Active => appearance(theme),
                            button::Status::Hovered => button::Style {
                                // background: Some(Background::Color(
                                //     theme.current_container().component.hover.into(),
                                // )),
                                border: Border {
                                    // radius: theme.cosmic().radius_xl().into(),
                                    radius: 20.0.into(),
                                    ..Default::default()
                                },
                                background: Some(Background::Color(
                                    Color {
                                        r: 1.0,
                                        g: 1.0,
                                        b: 1.0,
                                        a: 0.75,
                                    }
                                )),
                                ..appearance(theme)
                            },
                            button::Status::Pressed => appearance(theme),
                            button::Status::Disabled => appearance(theme),
                        },
                    ))
                    // cosmic::theme::iced::Button::Primary
                } else if w.state.contains(ext_workspace_handle_v1::State::Urgent) {
                    let appearance = |theme: &Theme| {
                        let cosmic = theme.cosmic();
                        button::Style {
                            background: Some(Background::Color(cosmic.palette.neutral_3.into())),
                            border: Border {
                                radius: cosmic.radius_xl().into(),
                                ..Default::default()
                            },
                            border_radius: theme.cosmic().radius_xl().into(),
                            text_color: theme.cosmic().destructive_button.base.into(),
                            ..button::Style::default()
                        }
                    };
                    cosmic::theme::iced::Button::Custom(Box::new(
                        move |theme, status| match status {
                            button::Status::Active => appearance(theme),
                            button::Status::Hovered => button::Style {
                                background: Some(Background::Color(
                                    theme.current_container().component.hover.into(),
                                )),
                                border: Border {
                                    radius: theme.cosmic().radius_xl().into(),
                                    ..Default::default()
                                },
                                ..appearance(theme)
                            },
                            button::Status::Pressed => appearance(theme),
                            button::Status::Disabled => appearance(theme),
                        },
                    ))
                } else {
                    let appearance = |theme: &Theme| {
                        let cosmic = theme.cosmic();
                        button::Style {
                            border: Border {
                                // radius: theme.cosmic().radius_xl().into(),
                                radius: 20.0.into(),
                                ..Default::default()
                            },
                            background: Some(Background::Color(
                                Color {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 0.5,
                                }
                            )),
                            // border_radius: cosmic.radius_xl().into(),
                            // text_color: theme.current_container().component.on.into(),
                            ..button::Style::default()
                        }
                    };
                    cosmic::theme::iced::Button::Custom(Box::new(
                        move |theme, status| match status {
                            button::Status::Active => appearance(theme),
                            button::Status::Hovered => button::Style {
                                background: Some(Background::Color(
                                    Color {
                                        r: 1.0,
                                        g: 1.0,
                                        b: 1.0,
                                        a: 0.25,
                                    }
                                )),
                                border: Border {
                                    radius: theme.cosmic().radius_xl().into(),
                                    ..Default::default()
                                },
                                ..appearance(theme)
                            },
                            button::Status::Pressed | button::Status::Disabled => appearance(theme),
                        },
                    ))
                },
            )
            .into()
        });
        // TODO if there is a popup_index, create a button with a popup for the remaining workspaces
        // Should it appear on hover or on click?
        // let layout_section: Element<_> = match self.layout {
        //     Layout::Row => row(buttons).spacing(4).into(),
        //     Layout::Column => column(buttons).spacing(4).into(),
        // };

        let layout_section: Element<_> = match self.layout {
            Layout::Row => row(buttons).spacing(8).align_y(Alignment::Center).into(),
            Layout::Column => column(buttons).spacing(8).align_x(Alignment::Center).into(),
        };
        // let mut limits = Limits::NONE.min_width(1.).min_height(1.);
        // if let Some(b) = self.core.applet.suggested_bounds {
        //     if b.width as i32 > 0 {
        //         limits = limits.max_width(b.width);
        //     }
        //     if b.height as i32 > 0 {
        //         limits = limits.max_height(b.height);
        //     }
        // }

        

        autosize::autosize(
            cosmic::widget::mouse_area(
                container(layout_section)
                    .padding(if horizontal {
                        [self.core.applet.suggested_padding(true).1, self.core.applet.suggested_padding(true).0]
                    } else {
                        [self.core.applet.suggested_padding(true).0, self.core.applet.suggested_padding(true).1]
                    })
                    // .padding([10.0, 15.0, 10.0, 15.0])
                    .style(|theme| {
                        let cosmic = theme.cosmic();
                        let bg_color = if self.hover {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.05)  // Hovered: darker bg
                        } else {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.0)  // Normal
                        };
                        container::Style {
                            background: Some(Background::Color(bg_color)),
                            border: Border {
                                radius: cosmic.radius_xl().into(),
                                ..Default::default()
                            },
                            ..container::Style::default()
                        }
                    })
            )
            .on_enter(Message::HoverEnter)
            .on_exit(Message::HoverExit)
            .on_press(Message::ContainerClick),
            AUTOSIZE_MAIN_ID.clone(),
        )
        // .limits(limits)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            workspaces().map(Message::WorkspaceUpdate),
            event::listen_with(|e, _, _| match e {
                Mouse(mouse::Event::WheelScrolled { delta }) => Some(Message::WheelScrolled(delta)),
                _ => None,
            }),
        ])
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}
