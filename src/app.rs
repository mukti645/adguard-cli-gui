use iced::{
    Alignment, Color, Element, Length, Task, Theme,
    widget::{
        button, column, container, row, scrollable, text, text_input,
        rule, space,
    },
};

use crate::cli::{self, Filter, LicenseInfo, ProtectionStatus, Status};
use crate::theme as cat;

// ─── Messages ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    TabSelected(Tab),

    // Status tab
    RefreshStatus,
    StatusLoaded(Status),
    ToggleProtection,
    ToggleDone(Result<String, String>),

    // License tab
    LicenseKeyChanged(String),
    ActivateLicense,
    ResetLicense,
    LicenseLoaded(LicenseInfo),
    LicenseActionDone(Result<String, String>),

    // Filters tab
    FiltersLoaded(Vec<Filter>),
    RefreshFilters,

    // Updates tab
    CheckUpdate,
    UpdateChecked(String),
    RunUpdate,
    UpdateDone(Result<String, String>),
    ExportLogs,
    LogsExported(Result<String, String>),

    // Install / download
    OpenDownloadPage,
    DownloadPageResult(Result<String, String>),

    // Configure
    OpenConfigure,

    // Notification dismiss
    DismissNotification,
}

// ─── Tabs ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Status,
    License,
    Filters,
    Updates,
}

// ─── App State ──────────────────────────────────────────────────────────────

pub struct AppState {
    tab: Tab,

    // Status
    status: Status,
    toggling: bool,

    // License
    license_key_input: String,
    license_info: LicenseInfo,
    license_loading: bool,

    // Filters
    filters: Vec<Filter>,
    filters_loading: bool,

    // Updates
    update_info: String,
    update_loading: bool,

    // Notification
    notification: Option<(String, bool)>, // (message, is_error)

    // Loading flags
    loading: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            tab: Tab::Status,
            status: Status {
                protection: ProtectionStatus::Unknown,
                version: String::new(),
                raw: String::new(),
            },
            toggling: false,
            license_key_input: String::new(),
            license_info: LicenseInfo {
                status: String::new(),
                key: String::new(),
                expires: String::new(),
                raw: String::new(),
            },
            license_loading: false,
            filters: vec![],
            filters_loading: false,
            update_info: String::new(),
            update_loading: false,
            notification: None,
            loading: false,
        }
    }
}

// ─── Application ────────────────────────────────────────────────────────────

pub fn run() -> iced::Result {
    iced::application(
        || (AppState::default(), Task::done(Message::RefreshStatus)),
        AppState::update,
        AppState::view,
    )
    .title(|_: &AppState| String::from("AdGuard CLI"))
    .theme(|_: &AppState| Theme::CatppuccinMocha)
    .window_size((800.0, 600.0))
    .run()
}

impl AppState {
    // ── Update ──────────────────────────────────────────────────────────────

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab) => {
                self.tab = tab.clone();
                match tab {
                    Tab::License => {
                        self.license_loading = true;
                        Task::perform(
                            async { tokio::task::spawn_blocking(cli::get_license).await.unwrap() },
                            Message::LicenseLoaded,
                        )
                    }
                    Tab::Filters => {
                        self.filters_loading = true;
                        Task::perform(
                            async { tokio::task::spawn_blocking(cli::list_filters).await.unwrap() },
                            Message::FiltersLoaded,
                        )
                    }
                    _ => Task::none(),
                }
            }

            Message::RefreshStatus => {
                self.loading = true;
                Task::perform(
                    async { tokio::task::spawn_blocking(cli::get_status).await.unwrap() },
                    Message::StatusLoaded,
                )
            }

            Message::StatusLoaded(status) => {
                self.loading = false;
                self.status = status;
                Task::none()
            }

            Message::ToggleProtection => {
                self.toggling = true;
                let is_running = self.status.protection == ProtectionStatus::Running;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            if is_running { cli::stop() } else { cli::start() }
                        })
                        .await
                        .unwrap()
                    },
                    Message::ToggleDone,
                )
            }

            Message::ToggleDone(result) => {
                self.toggling = false;
                match result {
                    Ok(msg) => self.notification = Some((msg, false)),
                    Err(e) => self.notification = Some((e, true)),
                }
                Task::done(Message::RefreshStatus)
            }

            Message::LicenseKeyChanged(key) => {
                self.license_key_input = key;
                Task::none()
            }

            Message::ActivateLicense => {
                let key = self.license_key_input.clone();
                if key.is_empty() {
                    self.notification = Some(("Please enter a license key".to_string(), true));
                    return Task::none();
                }
                self.license_loading = true;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || cli::activate_license(&key))
                            .await
                            .unwrap()
                    },
                    Message::LicenseActionDone,
                )
            }

            Message::ResetLicense => {
                self.license_loading = true;
                Task::perform(
                    async { tokio::task::spawn_blocking(cli::reset_license).await.unwrap() },
                    Message::LicenseActionDone,
                )
            }

            Message::LicenseLoaded(info) => {
                self.license_loading = false;
                self.license_info = info;
                Task::none()
            }

            Message::LicenseActionDone(result) => {
                self.license_loading = false;
                match result {
                    Ok(msg) => {
                        self.notification = Some((msg, false));
                        self.license_key_input.clear();
                    }
                    Err(e) => self.notification = Some((e, true)),
                }
                // Reload license info
                Task::perform(
                    async { tokio::task::spawn_blocking(cli::get_license).await.unwrap() },
                    Message::LicenseLoaded,
                )
            }

            Message::FiltersLoaded(filters) => {
                self.filters_loading = false;
                self.filters = filters;
                Task::none()
            }

            Message::RefreshFilters => {
                self.filters_loading = true;
                Task::perform(
                    async { tokio::task::spawn_blocking(cli::list_filters).await.unwrap() },
                    Message::FiltersLoaded,
                )
            }

            Message::CheckUpdate => {
                self.update_loading = true;
                Task::perform(
                    async { tokio::task::spawn_blocking(cli::check_update).await.unwrap() },
                    Message::UpdateChecked,
                )
            }

            Message::UpdateChecked(info) => {
                self.update_loading = false;
                self.update_info = info;
                Task::none()
            }

            Message::RunUpdate => {
                self.update_loading = true;
                Task::perform(
                    async { tokio::task::spawn_blocking(cli::update).await.unwrap() },
                    Message::UpdateDone,
                )
            }

            Message::UpdateDone(result) => {
                self.update_loading = false;
                match result {
                    Ok(msg) => self.notification = Some((msg, false)),
                    Err(e) => self.notification = Some((e, true)),
                }
                Task::none()
            }

            Message::ExportLogs => {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                let path = format!("{}/adguard-logs.zip", home);
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || cli::export_logs(&path))
                            .await
                            .unwrap()
                    },
                    Message::LogsExported,
                )
            }

            Message::LogsExported(result) => {
                match result {
                    Ok(msg) => self.notification = Some((msg, false)),
                    Err(e) => self.notification = Some((e, true)),
                }
                Task::none()
            }

            Message::OpenDownloadPage => {
                self.loading = true;
                self.notification = Some(("Opening download page...".to_string(), false));
                Task::perform(
                    async { tokio::task::spawn_blocking(cli::open_download_page).await.unwrap() },
                    Message::DownloadPageResult,
                )
            }

            Message::DownloadPageResult(result) => {
                self.loading = false;
                match result {
                    Ok(msg) => self.notification = Some((msg, false)),
                    Err(e) => self.notification = Some((e, true)),
                }
                Task::none()
            }

            Message::OpenConfigure => {
                tokio::task::spawn_blocking(cli::open_configure_terminal);
                self.notification = Some((
                    "Opening terminal with 'adguard-cli configure'...".to_string(),
                    false,
                ));
                Task::none()
            }

            Message::DismissNotification => {
                self.notification = None;
                Task::none()
            }
        }
    }

    // ── View ────────────────────────────────────────────────────────────────

    pub fn view(&self) -> Element<'_, Message> {
        let not_installed = self.status.protection == ProtectionStatus::NotInstalled;
        let not_configured = self.status.protection == ProtectionStatus::NotConfigured;

        let content = column![
            self.view_header(),
            self.view_tabs(),
            rule::horizontal(1),
            {
                let tab_content: Element<'_, Message> = if not_installed {
                    self.view_not_installed()
                } else if not_configured {
                    self.view_not_configured()
                } else {
                    match self.tab {
                        Tab::Status  => self.view_status(),
                        Tab::License => self.view_license(),
                        Tab::Filters => self.view_filters(),
                        Tab::Updates => self.view_updates(),
                    }
                };
                Element::from(container(tab_content)
                    .padding(20)
                    .width(Length::Fill))
            },
        ]
        .spacing(0);

        let with_notification: Element<'_, Message> = if let Some((msg, is_error)) = &self.notification {
            column![
                content,
                space::vertical().height(Length::Fill),
                self.view_notification(msg, *is_error),
            ]
            .into()
        } else {
            column![content].into()
        };

        container(with_notification)
            .style(|_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(cat::BASE)),
                ..Default::default()
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_header(&self) -> Element<'_, Message> {
        let status_color = match self.status.protection {
            ProtectionStatus::Running       => cat::GREEN,
            ProtectionStatus::Stopped       => cat::RED,
            ProtectionStatus::NotInstalled  => cat::YELLOW,
            ProtectionStatus::NotConfigured => cat::PEACH,
            ProtectionStatus::NoLicense     => cat::YELLOW,
            ProtectionStatus::Unknown       => cat::OVERLAY0,
        };

        let status_label = match self.status.protection {
            ProtectionStatus::Running       => "● Running",
            ProtectionStatus::Stopped       => "● Stopped",
            ProtectionStatus::NotInstalled  => "● Not installed",
            ProtectionStatus::NotConfigured => "● Not configured",
            ProtectionStatus::NoLicense     => "● No license",
            ProtectionStatus::Unknown       => "● Unknown",
        };

        let version_txt = if self.status.version.is_empty() {
            String::new()
        } else {
            format!("  {}", self.status.version)
        };

        row![
            column![
                text("AdGuard CLI")
                    .size(22)
                    .color(cat::MAUVE),
                text("Ad Blocker Control")
                    .size(12)
                    .color(cat::SUBTEXT0),
            ]
            .spacing(2),
            space::horizontal().width(Length::Fill),
            column![
                text(status_label).size(14).color(status_color),
                text(version_txt).size(11).color(cat::SUBTEXT0),
            ]
            .align_x(Alignment::End)
            .spacing(2),
        ]
        .align_y(Alignment::Center)
        .padding([16, 20])
        .into()
    }

    fn view_tabs(&self) -> Element<'_, Message> {
        let tabs = [
            (Tab::Status,  "Status"),
            (Tab::License, "License"),
            (Tab::Filters, "Filters"),
            (Tab::Updates, "Updates"),
        ];

        let tab_buttons: Vec<Element<'_, Message>> = tabs
            .iter()
            .map(|(tab, label)| {
                let active = &self.tab == tab;
                let color = if active { cat::MAUVE } else { cat::SUBTEXT0 };
                button(
                    text(*label).size(13).color(color)
                )
                .style(move |_theme, _status| button::Style {
                    background: if active {
                        Some(iced::Background::Color(cat::SURFACE0))
                    } else {
                        None
                    },
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                })
                .padding([6, 14])
                .on_press(Message::TabSelected(tab.clone()))
                .into()
            })
            .collect();

        container(
            row(tab_buttons).spacing(4).align_y(Alignment::Center)
        )
        .padding([4, 16])
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(cat::MANTLE)),
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
    }

    // ── Status Tab ──────────────────────────────────────────────────────────

    fn view_status(&self) -> Element<'_, Message> {
        let is_running = self.status.protection == ProtectionStatus::Running;

        let toggle_label = if self.toggling {
            if is_running { "Stopping..." } else { "Starting..." }
        } else if is_running {
            "Stop Protection"
        } else {
            "Start Protection"
        };

        let toggle_color = if is_running { cat::RED } else { cat::GREEN };
        let big_icon = match self.status.protection {
            ProtectionStatus::Running   => "🛡️ Protection ON",
            ProtectionStatus::NoLicense => "⚠ No License",
            _                           => "🔴 Protection OFF",
        };
        let big_color = match self.status.protection {
            ProtectionStatus::Running   => cat::GREEN,
            ProtectionStatus::NoLicense => cat::YELLOW,
            _                           => cat::RED,
        };

        let toggle_btn = button(
            text(toggle_label).size(14).color(Color::WHITE)
        )
        .style(move |_theme, _status| button::Style {
            background: Some(iced::Background::Color(toggle_color)),
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .padding([10, 24]);

        let toggle_btn = if !self.toggling {
            toggle_btn.on_press(Message::ToggleProtection)
        } else {
            toggle_btn
        };

        let refresh_btn = button(
            text("↻ Refresh").size(13).color(cat::BLUE)
        )
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(cat::SURFACE0)),
            border: iced::Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .padding([8, 16])
        .on_press(Message::RefreshStatus);

        let raw_output: Element<'_, Message> = if !self.status.raw.is_empty() {
            column![
                text("Status output:").size(12).color(cat::SUBTEXT0),
                container(
                    scrollable(
                        text(&self.status.raw).size(12).color(cat::TEXT)
                    )
                    .height(120)
                )
                .style(|_theme: &Theme| container::Style {
                    background: Some(iced::Background::Color(cat::MANTLE)),
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: cat::SURFACE1,
                    },
                    ..Default::default()
                })
                .padding(10)
                .width(Length::Fill),
            ]
            .spacing(6)
            .into()
        } else {
            space::vertical().height(0).into()
        };

        column![
            space::vertical().height(20),
            container(
                column![
                    text(big_icon).size(28).color(big_color),
                ]
                .align_x(Alignment::Center)
            )
            .center_x(Length::Fill),
            space::vertical().height(30),
            row![toggle_btn, refresh_btn]
                .spacing(12)
                .align_y(Alignment::Center),
            space::vertical().height(30),
            rule::horizontal(1),
            space::vertical().height(16),
            raw_output,
        ]
        .spacing(0)
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .into()
    }

    // ── License Tab ─────────────────────────────────────────────────────────

    fn view_license(&self) -> Element<'_, Message> {
        let license_status_color = match self.license_info.status.to_lowercase().as_str() {
            s if s.contains("active") || s.contains("valid") || s.contains("premium") => cat::GREEN,
            s if s.contains("trial") => cat::YELLOW,
            s if s.contains("free") => cat::BLUE,
            s if s.contains("no license") => cat::YELLOW,
            _ => cat::SUBTEXT0,
        };

        let current_info: Element<'_, Message> = if !self.license_info.raw.is_empty() {
            let mut info_rows: Vec<Element<'_, Message>> = vec![
                row![
                    text("Status: ").size(13).color(cat::SUBTEXT0),
                    text(&self.license_info.status).size(13).color(license_status_color),
                ]
                .spacing(4)
                .into(),
            ];
            if !self.license_info.key.is_empty() {
                info_rows.push(
                    row![
                        text("Key: ").size(13).color(cat::SUBTEXT0),
                        text(&self.license_info.key).size(13).color(cat::TEXT),
                    ]
                    .spacing(4)
                    .into(),
                );
            }
            if !self.license_info.expires.is_empty() {
                info_rows.push(
                    row![
                        text("Owner: ").size(13).color(cat::SUBTEXT0),
                        text(&self.license_info.expires).size(13).color(cat::TEXT),
                    ]
                    .spacing(4)
                    .into(),
                );
            }
            column![
                text("Current License").size(15).color(cat::TEXT),
                space::vertical().height(8),
                container(column(info_rows).spacing(6))
                    .style(|_theme: &Theme| container::Style {
                        background: Some(iced::Background::Color(cat::SURFACE0)),
                        border: iced::Border {
                            radius: 8.0.into(),
                            width: 1.0,
                            color: cat::SURFACE1,
                        },
                        ..Default::default()
                    })
                    .padding(14)
                    .width(Length::Fill),
            ]
            .spacing(0)
            .into()
        } else if self.license_loading {
            text("Loading license info...").size(13).color(cat::SUBTEXT0).into()
        } else {
            space::vertical().height(0).into()
        };

        let activate_section = column![
            text("Activate License").size(15).color(cat::TEXT),
            space::vertical().height(8),
            row![
                text_input("Enter license key...", &self.license_key_input)
                    .on_input(Message::LicenseKeyChanged)
                    .style(|_theme, _status| text_input::Style {
                        background: iced::Background::Color(cat::SURFACE0),
                        border: iced::Border {
                            radius: 6.0.into(),
                            width: 1.0,
                            color: cat::SURFACE1,
                        },
                        icon: cat::OVERLAY0,
                        placeholder: cat::OVERLAY0,
                        value: cat::TEXT,
                        selection: cat::MAUVE,
                    })
                    .padding(10)
                    .size(13),
                button(
                    text("Activate").size(13).color(Color::WHITE)
                )
                .style(|_theme, _status| button::Style {
                    background: Some(iced::Background::Color(cat::GREEN)),
                    border: iced::Border { radius: 6.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .padding([10, 18])
                .on_press(Message::ActivateLicense),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(0);

        let reset_btn = button(
            text("Reset License").size(13).color(cat::RED)
        )
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(cat::SURFACE0)),
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: cat::RED,
            },
            ..Default::default()
        })
        .padding([8, 16])
        .on_press(Message::ResetLicense);

        column![
            current_info,
            space::vertical().height(24),
            activate_section,
            space::vertical().height(16),
            reset_btn,
        ]
        .spacing(0)
        .width(Length::Fill)
        .into()
    }

    // ── Filters Tab ─────────────────────────────────────────────────────────

    fn view_filters(&self) -> Element<'_, Message> {
        let refresh_btn = button(
            text("↻ Refresh").size(13).color(cat::BLUE)
        )
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(cat::SURFACE0)),
            border: iced::Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        })
        .padding([8, 16])
        .on_press(Message::RefreshFilters);

        let header = row![
            text("Filters").size(15).color(cat::TEXT),
            space::horizontal().width(Length::Fill),
            refresh_btn,
        ]
        .align_y(Alignment::Center);

        let body: Element<'_, Message> = if self.filters_loading {
            container(
                text("Loading filters...").size(13).color(cat::SUBTEXT0)
            )
            .center_x(Length::Fill)
            .padding(30)
            .into()
        } else if self.filters.is_empty() {
            container(
                column![
                    text("No filters found").size(14).color(cat::SUBTEXT0),
                    space::vertical().height(8),
                    text("Filters require an active license. Use the License tab to activate one.")
                        .size(12).color(cat::OVERLAY0),
                ]
                .align_x(Alignment::Center)
                .spacing(0)
            )
            .center_x(Length::Fill)
            .padding(30)
            .into()
        } else {
            let items: Vec<Element<'_, Message>> = self
                .filters
                .iter()
                .map(|f| {
                    let dot_color = if f.enabled { cat::GREEN } else { cat::RED };
                    let dot = if f.enabled { "●" } else { "○" };
                    container(
                        row![
                            text(dot).size(14).color(dot_color),
                            text(&f.name).size(13).color(cat::TEXT),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center)
                    )
                    .style(|_theme| container::Style {
                        background: Some(iced::Background::Color(cat::SURFACE0)),
                        border: iced::Border {
                            radius: 6.0.into(),
                            width: 1.0,
                            color: cat::SURFACE1,
                        },
                        ..Default::default()
                    })
                    .padding([8, 12])
                    .width(Length::Fill)
                    .into()
                })
                .collect();

            scrollable(
                column(items).spacing(6)
            )
            .height(400)
            .into()
        };

        column![header, space::vertical().height(12), body]
            .spacing(0)
            .width(Length::Fill)
            .into()
    }

    // ── Updates Tab ─────────────────────────────────────────────────────────

    fn view_updates(&self) -> Element<'_, Message> {
        let check_btn = button(
            text(if self.update_loading { "Checking..." } else { "Check for updates" })
                .size(13)
                .color(cat::BLUE)
        )
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(cat::SURFACE0)),
            border: iced::Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        })
        .padding([8, 16]);

        let check_btn = if !self.update_loading {
            check_btn.on_press(Message::CheckUpdate)
        } else {
            check_btn
        };

        let update_btn = button(
            text("Run update").size(13).color(Color::WHITE)
        )
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(cat::GREEN)),
            border: iced::Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        })
        .padding([8, 16]);

        let update_btn = if !self.update_loading {
            update_btn.on_press(Message::RunUpdate)
        } else {
            update_btn
        };

        let logs_btn = button(
            text("Export logs").size(13).color(cat::PEACH)
        )
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(cat::SURFACE0)),
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: cat::PEACH,
            },
            ..Default::default()
        })
        .padding([8, 16])
        .on_press(Message::ExportLogs);

        let update_output: Element<'_, Message> = if !self.update_info.is_empty() {
            let out_box = container(
                scrollable(
                    text(&self.update_info).size(12).color(cat::TEXT)
                )
                .height(200)
            )
            .style(|_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(cat::MANTLE)),
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: cat::SURFACE1,
                },
                ..Default::default()
            })
            .padding(10)
            .width(Length::Fill);

            column![
                space::vertical().height(16),
                text("Output:").size(12).color(cat::SUBTEXT0),
                space::vertical().height(6),
                out_box,
            ]
            .spacing(0)
            .into()
        } else {
            space::vertical().height(0).into()
        };

        column![
            text("Updates & Logs").size(15).color(cat::TEXT),
            space::vertical().height(16),
            row![check_btn, update_btn, logs_btn].spacing(10),
            update_output,
        ]
        .spacing(0)
        .width(Length::Fill)
        .into()
    }

    // ── Not configured ──────────────────────────────────────────────────────

    fn view_not_configured(&self) -> Element<'_, Message> {
        let configure_btn = button(
            text("▶ Run adguard-cli configure").size(14).color(Color::WHITE)
        )
        .style(|_theme: &Theme, _status| button::Style {
            background: Some(iced::Background::Color(cat::PEACH)),
            border: iced::Border { radius: 8.0.into(), ..Default::default() },
            ..Default::default()
        })
        .padding([12, 24])
        .on_press(Message::OpenConfigure);

        let refresh_btn = button(
            text("↻ Refresh").size(13).color(cat::BLUE)
        )
        .style(|_theme: &Theme, _status| button::Style {
            background: Some(iced::Background::Color(cat::SURFACE0)),
            border: iced::Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        })
        .padding([8, 16])
        .on_press(Message::RefreshStatus);

        container(
            column![
                text("⚙ AdGuard CLI not configured").size(18).color(cat::PEACH),
                space::vertical().height(12),
                text("Run the configuration wizard to set up AdGuard CLI.")
                    .size(13).color(cat::SUBTEXT0),
                text("Choose proxy mode, DNS settings, and install certificates.")
                    .size(13).color(cat::SUBTEXT0),
                space::vertical().height(24),
                text("Or run manually in terminal:").size(13).color(cat::SUBTEXT0),
                container(
                    text("adguard-cli configure").size(13).color(cat::TEAL)
                )
                .style(|_theme: &Theme| container::Style {
                    background: Some(iced::Background::Color(cat::MANTLE)),
                    border: iced::Border { radius: 6.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .padding([8, 14]),
                space::vertical().height(24),
                row![configure_btn, refresh_btn].spacing(12),
            ]
            .align_x(Alignment::Center)
            .spacing(6)
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    // ── Not installed ───────────────────────────────────────────────────────

    fn view_not_installed(&self) -> Element<'_, Message> {
        let download_btn = button(
            text("⬇ Download from GitHub").size(14).color(Color::WHITE)
        )
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(cat::MAUVE)),
            border: iced::Border { radius: 8.0.into(), ..Default::default() },
            ..Default::default()
        })
        .padding([12, 24]);

        let download_btn = if !self.loading {
            download_btn.on_press(Message::OpenDownloadPage)
        } else {
            download_btn
        };

        let refresh_btn = button(
            text("↻ Check again").size(13).color(cat::BLUE)
        )
        .style(|_theme: &Theme, _status| button::Style {
            background: Some(iced::Background::Color(cat::SURFACE0)),
            border: iced::Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        })
        .padding([8, 16])
        .on_press(Message::RefreshStatus);

        let url = "https://github.com/AdguardTeam/AdGuardCLI/releases";

        container(
            column![
                text("⚠ AdGuard CLI not installed").size(18).color(cat::YELLOW),
                space::vertical().height(12),
                text("adguard-cli is required to run this application.")
                    .size(13)
                    .color(cat::SUBTEXT0),
                text("Download the latest release for your platform from GitHub:")
                    .size(13)
                    .color(cat::SUBTEXT0),
                space::vertical().height(12),
                container(
                    text(url).size(13).color(cat::TEAL)
                )
                .style(|_theme: &Theme| container::Style {
                    background: Some(iced::Background::Color(cat::MANTLE)),
                    border: iced::Border { radius: 6.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .padding([8, 14]),
                space::vertical().height(24),
                row![download_btn, refresh_btn].spacing(12),
            ]
            .align_x(Alignment::Center)
            .spacing(6)
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    // ── Notification bar ────────────────────────────────────────────────────

    fn view_notification<'a>(&self, msg: &'a str, is_error: bool) -> Element<'a, Message> {
        let bg = if is_error { cat::RED } else { cat::GREEN };
        let fg = Color::WHITE;

        // Truncate display message to prevent overflow (max ~120 chars)
        let display_msg: &str = if msg.len() > 120 { &msg[..120] } else { msg };

        container(
            row![
                text(display_msg).size(13).color(fg),
                space::horizontal().width(Length::Fill),
                button(text("✕").size(12).color(fg))
                    .style(|_theme, _status| button::Style {
                        background: None,
                        ..Default::default()
                    })
                    .padding([2, 6])
                    .on_press(Message::DismissNotification),
            ]
            .align_y(Alignment::Center)
        )
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(bg)),
            ..Default::default()
        })
        .padding([10, 16])
        .width(Length::Fill)
        .into()
    }
}
