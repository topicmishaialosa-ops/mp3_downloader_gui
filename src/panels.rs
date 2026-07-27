use std::time::Duration;

use eframe::egui::{self, Color32, Frame, Margin, Rounding, Stroke, Vec2};

use crate::batch;
use crate::player::{LoopMode, PlaylistItem};
use crate::theme::AppTheme;
use crate::types::*;
use crate::LinkParserApp;

impl eframe::App for LinkParserApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let theme = self.theme();
        theme.apply(ctx);
        self.drain_log_messages();
        self.check_loading_watchdog();

        if let Some(rx) = &self.rx {
            if let Ok(result) = rx.try_recv() {
                self.processed += 1;
                match result {
                    ParseResult::Success(track) => {
                        self.tracks.push(track);
                    }
                    ParseResult::SearchResults(results) => {
                        self.tracks.extend(results);
                    }
                    ParseResult::Error(_url, err) => {
                        self.error_count += 1;
                        self.last_error = Some(err);
                    }
                }
                if self.processed >= self.total_urls {
                    self.finish_loading();
                } else {
                    self.status = format!("⏳ {}/{}", self.processed, self.total_urls);
                }
                ctx.request_repaint();
            }
        }

        if self.has_active_downloads() {
            ctx.request_repaint();
        }

        if let Some(rx) = self.stream_rx.take() {
            if let Ok(result) = rx.try_recv() {
                self.loading = false;
                match result {
                    Ok((url, title, sub, is_video)) => {
                        if let Err(e) = self.player.play_url(&url, &title, &sub, is_video) {
                            self.status = format!("❌ Плеер: {}", e);
                        } else {
                            self.status = format!("🎧 {}", title);
                        }
                    }
                    Err(e) => self.status = format!("❌ Стрим: {}", e),
                }
                ctx.request_repaint();
            } else {
                self.stream_rx = Some(rx);
            }
        }

        if self.player.state.has_media {
            self.player.tick();
            ctx.request_repaint_after(Duration::from_millis(400));
        }

        egui::TopBottomPanel::bottom("player_bar")
            .frame(Frame {
                fill: theme.card_bg,
                inner_margin: Margin::symmetric(12.0, 8.0),
                ..Default::default()
            })
            .show(ctx, |ui| {
                self.show_player_bar(ui, theme);
            });

        egui::TopBottomPanel::top("header")
            .frame(Frame {
                fill: theme.header_bg,
                inner_margin: Margin::symmetric(16.0, 12.0),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("🎵 MP3Party")
                            .size(20.0)
                            .color(theme.text_on_header)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("загрузка треков")
                            .size(13.0)
                            .color(theme.text_on_header.gamma_multiply(0.75)),
                    );
                    if self.loading {
                        ui.add_space(8.0);
                        ui.add(egui::Spinner::new().color(theme.text_on_header));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(theme.header_button("✕"))
                            .on_hover_text("Закрыть")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }

                        let theme_label = if self.is_dark_mode {
                            "☀️ Светлая"
                        } else {
                            "🌙 Тёмная"
                        };
                        if ui.add(theme.header_button(theme_label)).clicked() {
                            self.is_dark_mode = !self.is_dark_mode;
                        }

                        let active_count = self
                            .download_tasks
                            .iter()
                            .filter(|t| {
                                matches!(
                                    *t.status.lock().unwrap(),
                                    DownloadStatus::Downloading { .. }
                                )
                            })
                            .count();
                        let dl_text = if active_count > 0 {
                            format!("📥 Загрузки ({})", active_count)
                        } else {
                            "📥 Загрузки".to_string()
                        };
                        if ui.add(theme.header_button(&dl_text)).clicked() {
                            self.show_downloads = !self.show_downloads;
                        }

                        let logs_text = if self.log_lines.is_empty() {
                            "📋 Логи".to_string()
                        } else {
                            format!("📋 Логи ({})", self.log_lines.len())
                        };
                        if ui.add(theme.header_button(&logs_text)).clicked() {
                            self.show_logs = !self.show_logs;
                        }
                    });
                });
            });

        if self.show_downloads {
            egui::Window::new("📥 Загрузки")
                .id("download_window".into())
                .resizable(true)
                .default_size([520.0, 320.0])
                .collapsible(true)
                .frame(
                    Frame::window(&ctx.style())
                        .fill(theme.card_bg)
                        .stroke(Stroke::new(1.0, theme.card_border))
                        .rounding(Rounding::same(10.0)),
                )
                .show(ctx, |ui| {
                    self.show_downloads_panel(ui, theme);
                });
        }

        if self.show_logs {
            egui::Window::new("📋 Логи")
                .id("logs_window".into())
                .resizable(true)
                .default_size([560.0, 360.0])
                .collapsible(true)
                .frame(
                    Frame::window(&ctx.style())
                        .fill(theme.card_bg)
                        .stroke(Stroke::new(1.0, theme.card_border))
                        .rounding(Rounding::same(10.0)),
                )
                .show(ctx, |ui| {
                    self.show_logs_panel(ui, theme);
                });
        }

        if self.show_batch_window {
            egui::Window::new("📋 Пакетный поиск")
                .id("batch_window".into())
                .resizable(true)
                .default_size([560.0, 420.0])
                .collapsible(true)
                .frame(
                    Frame::window(&ctx.style())
                        .fill(theme.card_bg)
                        .stroke(Stroke::new(1.0, theme.card_border))
                        .rounding(Rounding::same(10.0)),
                )
                .show(ctx, |ui| {
                    self.show_batch_panel(ui, theme);
                });
        }

        if self.show_playlist_window {
            egui::Window::new("📋 Плейлист")
                .id("playlist_window".into())
                .resizable(true)
                .default_size([420.0, 360.0])
                .collapsible(true)
                .frame(
                    Frame::window(&ctx.style())
                        .fill(theme.card_bg)
                        .stroke(Stroke::new(1.0, theme.card_border))
                        .rounding(Rounding::same(10.0)),
                )
                .show(ctx, |ui| {
                    self.show_playlist_panel(ui, theme);
                });
        }

        ctx.input(|i| {
            if self.player.state.has_media {
                if i.key_pressed(egui::Key::Space) && !i.modifiers.ctrl {
                    self.player.toggle_pause();
                }
                if i.key_pressed(egui::Key::ArrowLeft) && !i.modifiers.ctrl {
                    let new_pos = (self.player.state.position_secs - 5.0).max(0.0);
                    self.player_seek_request = Some(new_pos);
                }
                if i.key_pressed(egui::Key::ArrowRight) && !i.modifiers.ctrl {
                    let new_pos = self.player.state.position_secs + 5.0;
                    self.player_seek_request = Some(new_pos);
                }
                if i.key_pressed(egui::Key::ArrowUp) {
                    let new_vol = (self.player.volume + 0.05).min(1.0);
                    self.player_volume_request = Some(new_vol);
                }
                if i.key_pressed(egui::Key::ArrowDown) {
                    let new_vol = (self.player.volume - 0.05).max(0.0);
                    self.player_volume_request = Some(new_vol);
                }
            }
            if i.key_pressed(egui::Key::ArrowRight) && i.modifiers.ctrl {
                self.player.play_next();
            }
            if i.key_pressed(egui::Key::ArrowLeft) && i.modifiers.ctrl {
                self.player.play_prev();
            }
            if i.key_pressed(egui::Key::S) && !i.modifiers.ctrl && !i.modifiers.alt {
                self.player.toggle_shuffle();
            }
        });

        egui::CentralPanel::default()
            .frame(Frame {
                fill: theme.window_bg,
                inner_margin: Margin::same(16.0),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.main_tab == MainTab::Search, "🔎 Поиск")
                        .clicked()
                    {
                        self.main_tab = MainTab::Search;
                    }
                    if ui
                        .selectable_label(self.main_tab == MainTab::Library, "📂 Мои файлы")
                        .clicked()
                    {
                        self.main_tab = MainTab::Library;
                        self.refresh_library();
                    }
                });
                ui.add_space(8.0);
                match self.main_tab {
                    MainTab::Search => self.show_main_panel(ui, theme),
                    MainTab::Library => self.show_library_panel(ui, theme),
                }
            });
    }
}

impl LinkParserApp {
    pub fn show_player_bar(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        if !self.player.state.has_media {
            return;
        }
        self.player.tick();
        if let Some(seek) = self.player_seek_request.take() {
            self.player.seek_to(seek);
        }
        if let Some(vol) = self.player_volume_request.take() {
            self.player.set_volume(vol);
        }

        theme.card().show(ui, |ui| {
            ui.horizontal(|ui| {
                let icon = if self.player.state.is_playing {
                    "⏸"
                } else {
                    "▶"
                };
                if ui.add(theme.primary_button(icon)).clicked() {
                    self.player.toggle_pause();
                }
                if ui.add(theme.neutral_button("⏹")).clicked() {
                    self.player.stop();
                }
                if ui.add(theme.neutral_button("⏮")).clicked() {
                    self.player.play_prev();
                }
                if ui.add(theme.neutral_button("⏭")).clicked() {
                    self.player.play_next();
                }
                ui.vertical(|ui| {
                    let playlist_info = if self.player.playlist.len() > 1 {
                        format!(
                            "  [{}/{}]",
                            self.player.playlist_index + 1,
                            self.player.playlist.len()
                        )
                    } else {
                        String::new()
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "{}{}",
                            &self.player.state.title, playlist_info
                        ))
                        .strong()
                        .color(theme.text_primary),
                    );
                    ui.label(
                        egui::RichText::new(&self.player.state.subtitle)
                            .size(11.0)
                            .color(theme.text_muted),
                    );
                });

                let shuffle_btn = if self.player.shuffle {
                    theme.primary_button("🔀")
                } else {
                    theme.neutral_button("🔀")
                };
                if ui
                    .add(shuffle_btn)
                    .on_hover_text(if self.player.shuffle {
                        "Перемешать: ВКЛ"
                    } else {
                        "Перемешать: ВЫКЛ"
                    })
                    .clicked()
                {
                    self.player.toggle_shuffle();
                }

                let loop_label = match self.player.loop_mode {
                    LoopMode::NoRepeat => "🔁",
                    LoopMode::RepeatAll => "🔁",
                    LoopMode::RepeatOne => "🔂",
                };
                let loop_active = self.player.loop_mode != LoopMode::NoRepeat;
                let loop_btn = if loop_active {
                    theme.primary_button(loop_label)
                } else {
                    theme.neutral_button(loop_label)
                };
                let loop_resp = ui.add(loop_btn);
                if loop_resp.on_hover_text(self.player.loop_mode.label()).clicked() {
                    self.player.set_loop_mode(self.player.loop_mode.next());
                }

                let vol = self.player.volume();
                let vol_label = format!("🔊 {}", (vol * 100.0) as u32);
                let vol_resp = ui.add(
                    egui::Slider::new(&mut self.player.volume, 0.0..=1.0)
                        .clamping(egui::SliderClamping::Always)
                        .show_value(false)
                        .text(vol_label),
                );
                if vol_resp.changed() {
                    self.player.set_volume(self.player.volume());
                }

                let pl_text = if self.player.playlist.len() > 1 {
                    format!("📋 ({})", self.player.playlist.len())
                } else {
                    "📋".to_string()
                };
                if ui
                    .add(theme.neutral_button(&pl_text))
                    .on_hover_text("Плейлист")
                    .clicked()
                {
                    self.show_playlist_window = !self.show_playlist_window;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").clicked() {
                        self.player.stop();
                    }
                });
            });
            if self.player.state.duration_secs > 0.0 || self.player.state.position_secs > 0.0 {
                let dur = self.player.state.duration_secs.max(1.0);
                let mut pos = self.player.state.position_secs.min(dur);
                let pos_label = Self::format_duration(pos);
                if ui
                    .add(
                        egui::Slider::new(&mut pos, 0.0..=dur)
                            .text(pos_label)
                            .trailing_fill(true),
                    )
                    .changed()
                {
                    self.player_seek_request = Some(pos);
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{} / {}",
                        Self::format_duration(self.player.state.position_secs),
                        Self::format_duration(dur)
                    ))
                    .size(11.0)
                    .color(theme.text_muted),
                );
            }
        });
    }

    pub fn show_library_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        theme.card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme.section_title("📂 Мои файлы"));
                if ui.add(theme.neutral_button("🔄 Обновить")).clicked() {
                    self.refresh_library();
                }
                if ui.add(theme.neutral_button("📂 Открыть папку")).clicked() {
                    crate::player::open_folder_in_file_manager(&self.downloads_folder);
                }
            });
            ui.label(
                egui::RichText::new(self.downloads_folder.display().to_string())
                    .size(11.0)
                    .color(theme.text_muted),
            );
            ui.add_space(6.0);

            if self.library_files.is_empty() {
                ui.label(egui::RichText::new("Скачанных файлов пока нет").color(theme.text_muted));
                return;
            }

            egui::ScrollArea::vertical()
                .max_height(ui.available_height())
                .show(ui, |ui| {
                        for f in &self.library_files {
                        ui.horizontal(|ui| {
                            let icon = if f.is_video { "🎬" } else { "🎵" };
                            ui.label(icon);
                            ui.vertical(|ui| {
                                ui.label(&f.display_name);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} · {}",
                                        Self::format_bytes(f.size_bytes),
                                        f.path.file_name().unwrap_or_default().to_string_lossy()
                                    ))
                                    .size(10.0)
                                    .color(theme.text_muted),
                                );
                            });
                            if ui.add(theme.primary_button("▶")).clicked() {
                                self.player.stop();
                                let item = PlaylistItem {
                                    path_or_url: f.path.to_string_lossy().to_string(),
                                    title: f.display_name.clone(),
                                    subtitle: if f.is_video { "Видео".into() } else { "Локальный файл".into() },
                                    is_video: f.is_video,
                                    is_url: false,
                                };
                                self.player.playlist.clear();
                                self.player.playlist.push_back(item);
                                self.player.playlist_index = 0;
                                self.player.play_current();
                            }
                            if ui.add(theme.neutral_button("➕")).on_hover_text("В плейлист").clicked() {
                                let item = PlaylistItem {
                                    path_or_url: f.path.to_string_lossy().to_string(),
                                    title: f.display_name.clone(),
                                    subtitle: if f.is_video { "Видео".into() } else { "Локальный файл".into() },
                                    is_video: f.is_video,
                                    is_url: false,
                                };
                                self.player.add_to_playlist(item);
                                self.status = format!("➕ {} добавлен в плейлист", f.display_name);
                            }
                        });
                        ui.separator();
                    }
                });
        });
    }

    pub fn show_download_progress(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        let active: Vec<_> = self
            .download_tasks
            .iter()
            .filter_map(|t| {
                let s = t.status.lock().unwrap().clone();
                match s {
                    DownloadStatus::Pending => {
                        Some((t.track.clone(), t.source, t.ytdlp_format, None))
                    }
                    DownloadStatus::Downloading {
                        progress,
                        bytes,
                        total,
                    } => Some((
                        t.track.clone(),
                        t.source,
                        t.ytdlp_format,
                        Some((progress, bytes, total)),
                    )),
                    _ => None,
                }
            })
            .collect();

        if active.is_empty() {
            return;
        }

        theme.card().show(ui, |ui| {
            let folder_hint = self.downloads_folder.display().to_string();
            ui.horizontal(|ui| {
                ui.label(theme.section_title("📥 Скачивание"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(theme.neutral_button("📂 Открыть папку"))
                        .on_hover_text(&folder_hint)
                        .clicked()
                    {
                        self.open_downloads_folder();
                    }
                });
            });

            ui.add_space(6.0);

            let mut total_bytes: u64 = 0;
            let mut total_size: u64 = 0;

            for (track, source, ytdlp_fmt, prog) in &active {
                ui.label(
                    egui::RichText::new(format!(
                        "[{}] {} — {}",
                        Self::task_source_label(*source, *ytdlp_fmt),
                        track.artist,
                        track.title
                    ))
                    .size(12.0)
                    .strong()
                    .color(theme.text_primary),
                );

                match prog {
                    None => {
                        ui.add(
                            egui::ProgressBar::new(0.0)
                                .animate(true)
                                .text("⏳ Подготовка…")
                                .desired_width(ui.available_width()),
                        );
                    }
                    Some((progress, bytes, total)) => {
                        if *total > 0 {
                            total_bytes += bytes;
                            total_size += total;
                            ui.add(
                                egui::ProgressBar::new(*progress)
                                    .show_percentage()
                                    .fill(theme.progress)
                                    .desired_width(ui.available_width())
                                    .text(format!(
                                        "{} / {}",
                                        Self::format_bytes(*bytes),
                                        Self::format_bytes(*total)
                                    )),
                            );
                        } else {
                            total_bytes += bytes;
                            ui.add(
                                egui::ProgressBar::new(0.0)
                                    .animate(true)
                                    .fill(theme.progress)
                                    .desired_width(ui.available_width())
                                    .text(format!("{} скачано", Self::format_bytes(*bytes))),
                            );
                        }
                    }
                }
                ui.add_space(6.0);
            }

            if total_size > 0 {
                let overall = total_bytes as f32 / total_size as f32;
                ui.separator();
                ui.label(
                    egui::RichText::new("Общий прогресс")
                        .size(11.0)
                        .color(theme.text_muted),
                );
                ui.add(
                    egui::ProgressBar::new(overall.min(1.0))
                        .show_percentage()
                        .fill(theme.accent)
                        .desired_width(ui.available_width())
                        .text(format!(
                            "{} / {}",
                            Self::format_bytes(total_bytes),
                            Self::format_bytes(total_size)
                        )),
                );
            }
        });
    }

    pub fn show_batch_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        ui.label(
            egui::RichText::new(
                "Введите по одному треку на строку.\n\
                 Формат: «Исполнитель - Название», «Название» (без разделителя), или URL.\n\
                 Нумерация («1. », «12) ») и комментарии после «#» игнорируются.\n\
                 Источник: ",
            )
            .size(12.0)
            .color(theme.text_secondary),
        );
        ui.label(
            egui::RichText::new(self.download_source.label())
                .size(13.0)
                .strong()
                .color(theme.accent),
        );
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.batch_input)
                        .desired_rows(8)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("Кино - Группа крови\nАгата Кристи - Опиум для никого\nСектор Газа - Лирика\nhttps://www.youtube.com/watch?v=…"),
                );
            });
        ui.add_space(6.0);
        let parsed = batch::parse_batch_queries(&self.batch_input);
        ui.label(
            egui::RichText::new(format!("Будет отправлено запросов: {}", parsed.len()))
                .size(11.0)
                .color(theme.text_muted),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.loading, theme.success_button("▶ Найти по списку"))
                .clicked()
            {
                self.start_batch_search();
                self.show_batch_window = false;
            }
            if ui.add(theme.neutral_button("Очистить")).clicked() {
                self.batch_input.clear();
            }
            if ui.add(theme.neutral_button("Закрыть")).clicked() {
                self.show_batch_window = false;
            }
        });
    }

    pub fn show_main_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        theme.card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Источник (поиск и скачивание):")
                        .size(13.0)
                        .strong()
                        .color(theme.text_primary),
                );
                egui::ComboBox::from_id_salt("download_source")
                    .selected_text(self.download_source.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.download_source,
                            DownloadSource::Mp3Party,
                            DownloadSource::Mp3Party.label(),
                        );
                        ui.selectable_value(
                            &mut self.download_source,
                            DownloadSource::DriveMusic,
                            DownloadSource::DriveMusic.label(),
                        );
                        ui.selectable_value(
                            &mut self.download_source,
                            DownloadSource::YtDlp,
                            DownloadSource::YtDlp.label(),
                        );
                    });
            });
            if self.download_source == DownloadSource::YtDlp {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Формат yt-dlp:")
                            .size(12.0)
                            .color(theme.text_secondary),
                    );
                    egui::ComboBox::from_id_salt("ytdlp_format")
                        .selected_text(self.ytdlp_format.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.ytdlp_format,
                                YtDlpFormat::Mp3,
                                YtDlpFormat::Mp3.label(),
                            );
                            ui.selectable_value(
                                &mut self.ytdlp_format,
                                YtDlpFormat::Mp4,
                                YtDlpFormat::Mp4.label(),
                            );
                        });
                });
            }

            ui.add_space(4.0);
            let hint = match self.download_source {
                DownloadSource::Mp3Party => "Поиск на mp3party.net, скачивание online/download URL",
                DownloadSource::DriveMusic => {
                    "Поиск на drivemusic.me; ссылки на MP3 временные — скачивание со страницы трека"
                }
                DownloadSource::YtDlp => match self.ytdlp_format {
                    YtDlpFormat::Mp3 => {
                        "YouTube: поиск ytsearch, скачивание MP3 (-x --audio-format mp3)"
                    }
                    YtDlpFormat::Mp4 => "YouTube: поиск ytsearch, скачивание MP4 (видео+аудио)",
                },
            };
            ui.label(egui::RichText::new(hint).size(11.0).color(theme.text_muted));
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.columns(2, |cols| {
                theme.card().show(&mut cols[0], |ui| {
                    ui.label(theme.section_title("📎 Парсинг ссылок"));
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Вставьте ссылки или ID треков (по одному на строку)")
                            .size(12.0)
                            .color(theme.text_muted),
                    );
                    ui.add_space(4.0);

                    let input_h = 72.0;
                    egui::ScrollArea::vertical()
                        .id_salt("input_scroll")
                        .max_height(input_h)
                        .show(ui, |ui| {
                            ui.add_sized(
                                [ui.available_width(), input_h],
                                egui::TextEdit::multiline(&mut self.input_text)
                                    .desired_width(f32::INFINITY)
                                    .font(egui::TextStyle::Monospace)
                                    .hint_text("https://dl2.mp3party.net/download/8787500"),
                            );
                        });
                });

                theme.card().show(&mut cols[1], |ui| {
                    let search_title = match self.download_source {
                        DownloadSource::Mp3Party => "🔎 Поиск (MP3Party)",
                        DownloadSource::DriveMusic => "🔎 Поиск (DriveMusic)",
                        DownloadSource::YtDlp => "🔎 Поиск (YouTube)",
                    };
                    ui.label(theme.section_title(search_title));
                    ui.add_space(6.0);
                    let search_hint = match self.download_source {
                        DownloadSource::Mp3Party => "Исполнитель или название на mp3party",
                        DownloadSource::DriveMusic => "Исполнитель или название на drivemusic",
                        DownloadSource::YtDlp => "Запрос для поиска на YouTube",
                    };
                    ui.label(
                        egui::RichText::new(search_hint)
                            .size(12.0)
                            .color(theme.text_muted),
                    );
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        let placeholder = match self.download_source {
                            DownloadSource::Mp3Party => "Queen, Кино…",
                            DownloadSource::DriveMusic => "Исполнитель или название…",
                            DownloadSource::YtDlp => "Queen Killer Queen…",
                        };
                        let search_resp = ui.add_sized(
                            [ui.available_width() - 100.0, 32.0],
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text(placeholder),
                        );

                        let search_clicked = ui
                            .add_enabled(!self.loading, theme.primary_button("🔍 Найти"))
                            .clicked();

                        if self.loading
                            && ui
                                .add_enabled(true, theme.neutral_button("⏹ Отмена"))
                                .clicked()
                        {
                            self.cancel_loading();
                        }

                        let enter_in_search = ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && search_resp.has_focus();

                        if search_clicked || enter_in_search {
                            self.start_search();
                        }
                    });

                    if ui
                        .add(theme.neutral_button("📋 Список"))
                        .on_hover_text(
                            "Пакетный поиск: по одному треку на строку\n(Исполнитель - Название)",
                        )
                        .clicked()
                    {
                        self.show_batch_window = true;
                    }
                });
            });
        });

        ui.add_space(10.0);

        theme.card().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!self.loading, theme.success_button("📥 Парсить ссылки"))
                    .clicked()
                {
                    self.tracks.clear();
                    self.output_mode = OutputMode::UrlParsing;
                    self.start_parsing();
                }

                if ui.add(theme.neutral_button("🗑 Очистить")).clicked() {
                    self.tracks.clear();
                    self.input_text.clear();
                    self.search_query.clear();
                    self.result_filter.clear();
                    self.output_mode = OutputMode::UrlParsing;
                    self.status = "✅ Очищено".into();
                }

                if !self.tracks.is_empty()
                    && ui.add(theme.neutral_button("📋 Копировать")).clicked()
                {
                    let mut text = String::from("ID\tИсполнитель\tНазвание\tСсылка\n");
                    for t in &self.tracks {
                        text.push_str(&format!("{}\t{}\t{}\t{}\n", t.id, t.artist, t.title, t.url));
                    }
                    ui.output_mut(|o| o.copied_text = text);
                    self.status = "📋 Скопировано в буфер".into();
                }

                if ui.add(theme.neutral_button("📂 Выбрать папку")).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Выберите папку для загрузок")
                        .pick_folder()
                    {
                        self.downloads_folder = path;
                        self.status = format!("📂 Папка: {}", self.downloads_folder.display());
                    }
                }

                if ui
                    .add(theme.neutral_button("📂 Открыть папку"))
                    .on_hover_text("Открыть папку загрузок в файловом менеджере")
                    .clicked()
                {
                    self.open_downloads_folder();
                }
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Сохранение в:")
                        .size(11.0)
                        .color(theme.text_muted),
                );
                if ui
                    .link(
                        egui::RichText::new(self.downloads_folder.display().to_string()).size(11.0),
                    )
                    .on_hover_text("Открыть папку")
                    .clicked()
                {
                    self.open_downloads_folder();
                }
            });
        });

        ui.add_space(8.0);

        theme.status_bar().show(ui, |ui| {
            ui.horizontal(|ui| {
                let color = theme.status_color(&self.status, self.loading);
                ui.colored_label(color, egui::RichText::new(&self.status).size(13.0));
            });
        });

        if self.has_active_downloads() {
            ui.add_space(8.0);
            self.show_download_progress(ui, theme);
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        if !self.tracks.is_empty() {
            let mode_label = match self.output_mode {
                OutputMode::UrlParsing => "по ссылкам".to_string(),
                OutputMode::Search => format!("поиск — {}", self.download_source.label()),
            };

            theme.card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        theme
                            .section_title(&format!(
                                "📊 Результаты ({}) — {} треков",
                                mode_label,
                                self.tracks.len()
                            ))
                            .size(14.0),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_sized(
                            [200.0, 26.0],
                            egui::TextEdit::singleline(&mut self.result_filter)
                                .hint_text("🔍 фильтр…"),
                        );
                        ui.label(
                            egui::RichText::new("Фильтр")
                                .size(12.0)
                                .color(theme.text_muted),
                        );

                        if ui
                            .add(theme.success_button("📥 Скачать все"))
                            .on_hover_text("Скачать все отфильтрованные треки")
                            .clicked()
                        {
                            let filtered = self.filtered_track_indices();
                            for idx in filtered {
                                self.start_download(idx);
                            }
                        }
                    });
                });

                let filtered_indices = self.filtered_track_indices();
                if !self.result_filter.trim().is_empty() {
                    ui.label(
                        egui::RichText::new(format!("Показано: {}", filtered_indices.len()))
                            .size(11.0)
                            .color(theme.text_muted),
                    );
                }

                ui.add_space(6.0);

                let avail = ui.available_height();
                let table_h = if self.show_downloads {
                    avail * 0.55
                } else {
                    avail
                };

                egui::ScrollArea::vertical()
                    .id_salt("results_scroll")
                    .auto_shrink([false; 2])
                    .max_height(table_h.max(120.0))
                    .show(ui, |ui| {
                        let mut to_remove: Option<usize> = None;
                        let mut to_download: Option<usize> = None;
                        let mut to_stream: Option<usize> = None;

                        egui::Grid::new(format!("results_grid_{:?}", self.output_mode))
                            .striped(true)
                            .spacing([12.0, 8.0])
                            .min_col_width(60.0)
                            .show(ui, |ui| {
                                ui.strong(
                                    egui::RichText::new("ID").size(12.0).color(theme.text_muted),
                                );
                                ui.strong(
                                    egui::RichText::new("Исполнитель")
                                        .size(12.0)
                                        .color(theme.text_muted),
                                );
                                ui.strong(
                                    egui::RichText::new("Название")
                                        .size(12.0)
                                        .color(theme.text_muted),
                                );
                                ui.strong(
                                    egui::RichText::new(" ").size(12.0).color(theme.text_muted),
                                );
                                ui.end_row();

                                for i in &filtered_indices {
                                    let track = &self.tracks[*i];

                                    if ui
                                        .link(
                                            egui::RichText::new(&track.id)
                                                .color(theme.link)
                                                .size(12.0),
                                        )
                                        .clicked()
                                    {
                                        ui.output_mut(|o| o.copied_text = track.id.clone());
                                        self.status = format!("📋 ID {} скопирован", track.id);
                                    }

                                    ui.label(
                                        egui::RichText::new(&track.artist)
                                            .color(theme.text_primary)
                                            .size(13.0),
                                    );

                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(&track.title)
                                                .color(theme.text_secondary)
                                                .size(13.0),
                                        );
                                        let src = self.download_source.label();
                                        if ui
                                            .add(
                                                theme
                                                    .success_button("⬇")
                                                    .min_size(Vec2::new(36.0, 24.0)),
                                            )
                                            .on_hover_text(format!("Скачать через {}", src))
                                            .clicked()
                                        {
                                            to_download = Some(*i);
                                        }
                                        if ui
                                            .add(
                                                theme
                                                    .neutral_button("▶")
                                                    .min_size(Vec2::new(36.0, 24.0)),
                                            )
                                            .on_hover_text("Слушать онлайн")
                                            .clicked()
                                        {
                                            to_stream = Some(*i);
                                        }
                                        if ui
                                            .add(
                                                theme
                                                    .neutral_button("➕")
                                                    .min_size(Vec2::new(36.0, 24.0)),
                                            )
                                            .on_hover_text("Добавить в плейлист")
                                            .clicked()
                                        {
                                            let track = &self.tracks[*i];
                                            let item = PlaylistItem {
                                                path_or_url: track.url.clone(),
                                                title: format!("{} — {}", track.artist, track.title),
                                                subtitle: format!("Стрим {}", self.download_source.label()),
                                                is_video: false,
                                                is_url: true,
                                            };
                                            self.player.add_to_playlist(item);
                                            self.status = format!("➕ {} добавлен в плейлист", track.title);
                                        }
                                    });

                                    if ui.small_button("✕").on_hover_text("Убрать").clicked()
                                    {
                                        to_remove = Some(*i);
                                    }
                                    ui.end_row();
                                }
                            });

                        if let Some(idx) = to_download {
                            self.start_download(idx);
                        }
                        if let Some(idx) = to_stream {
                            self.start_stream(idx);
                        }
                        if let Some(idx) = to_remove {
                            self.tracks.remove(idx);
                        }
                    });
            });
        } else if !self.loading {
            ui.add_space(32.0);
            ui.vertical_centered(|ui| {
                theme.card().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("🎧").size(36.0).color(theme.text_muted));
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Вставьте ссылки или найдите треки")
                                .size(15.0)
                                .color(theme.text_secondary),
                        );
                        ui.label(
                            egui::RichText::new("Результаты появятся здесь")
                                .size(12.0)
                                .color(theme.text_muted),
                        );
                    });
                });
            });
        }
    }

    pub fn show_logs_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        ui.horizontal(|ui| {
            ui.label(
                theme
                    .section_title(&format!("📋 {} строк", self.log_lines.len()))
                    .size(14.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme.neutral_button("📋 Копировать")).clicked() {
                    let text = self.log_lines.join("\n");
                    ui.output_mut(|o| o.copied_text = text);
                }
                if ui.add(theme.neutral_button("🗑 Очистить")).clicked() {
                    self.log_lines.clear();
                    self.push_log_line(format!("[{}] Лог очищен.", Self::log_timestamp()));
                }
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        if self.log_lines.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Лог пуст")
                        .size(14.0)
                        .color(theme.text_muted),
                );
            });
            return;
        }

        let scroll_height = ui.available_height().max(120.0);
        egui::ScrollArea::vertical()
            .id_salt("logs_scroll")
            .auto_shrink([false; 2])
            .max_height(scroll_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log_lines {
                    ui.label(
                        egui::RichText::new(line)
                            .size(12.0)
                            .family(egui::FontFamily::Monospace)
                            .color(theme.text_secondary),
                    );
                }
            });
    }

    pub fn show_downloads_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        ui.horizontal(|ui| {
            ui.label(
                theme
                    .section_title(&format!("📥 {} задач", self.download_tasks.len()))
                    .size(14.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme.neutral_button("📂 Открыть папку")).clicked() {
                    self.open_downloads_folder();
                }
                if ui.add(theme.neutral_button("🗑 Очистить готовые")).clicked() {
                    self.download_tasks.retain(|t| {
                        let s = t.status.lock().unwrap();
                        matches!(
                            *s,
                            DownloadStatus::Downloading { .. } | DownloadStatus::Pending
                        )
                    });
                }
                if self.has_active_downloads()
                    && ui
                        .add(theme.neutral_button("⏹ Остановить всё"))
                        .on_hover_text("Принудительно остановить все загрузки")
                        .clicked()
                {
                    self.cancel_all_downloads();
                }
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        if self.download_tasks.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Нет активных загрузок")
                        .size(14.0)
                        .color(theme.text_muted),
                );
            });
            return;
        }

        let mut to_remove: Vec<usize> = Vec::new();
        let mut to_cancel: Vec<usize> = Vec::new();
        let mut open_folder = false;

        let scroll_height = ui.available_height().max(100.0);
        egui::ScrollArea::vertical()
            .id_salt("download_scroll")
            .auto_shrink([false; 2])
            .max_height(scroll_height)
            .show(ui, |ui| {
                for (i, task) in self.download_tasks.iter().enumerate() {
                    let status = task.status.lock().unwrap().clone();
                    let track = &task.track;

                    theme.card().show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "[{}] {} — {}",
                                Self::task_source_label(task.source, task.ytdlp_format),
                                track.artist,
                                track.title
                            ))
                            .size(13.0)
                            .strong()
                            .color(theme.text_primary),
                        );

                        ui.add_space(6.0);

                        match &status {
                            DownloadStatus::Pending => {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::ProgressBar::new(0.0)
                                            .animate(true)
                                            .text("⏳ Ожидание…")
                                            .desired_width(ui.available_width() - 90.0),
                                    );
                                    if ui
                                        .add(theme.neutral_button("⏹ Стоп"))
                                        .on_hover_text("Принудительно остановить")
                                        .clicked()
                                    {
                                        to_cancel.push(i);
                                    }
                                });
                            }
                            DownloadStatus::Downloading {
                                progress,
                                bytes,
                                total,
                            } => {
                                ui.horizontal(|ui| {
                                    if *total > 0 {
                                        let pct = (progress * 100.0) as u32;
                                        ui.add(
                                            egui::ProgressBar::new(*progress)
                                                .show_percentage()
                                                .desired_width(ui.available_width() - 90.0)
                                                .fill(theme.progress)
                                                .text(format!(
                                                    "{} / {} ({}%)",
                                                    Self::format_bytes(*bytes),
                                                    Self::format_bytes(*total),
                                                    pct
                                                )),
                                        );
                                    } else {
                                        ui.add(
                                            egui::ProgressBar::new(*progress)
                                                .animate(*progress < 0.01)
                                                .fill(theme.progress)
                                                .desired_width(ui.available_width() - 90.0)
                                                .text(format!(
                                                    "{} скачано",
                                                    Self::format_bytes(*bytes)
                                                )),
                                        );
                                    }
                                    if ui
                                        .add(theme.neutral_button("⏹ Стоп"))
                                        .on_hover_text("Принудительно остановить")
                                        .clicked()
                                    {
                                        to_cancel.push(i);
                                    }
                                });
                            }
                            DownloadStatus::Cancelled => {
                                ui.horizontal(|ui| {
                                    ui.colored_label(theme.text_muted, "⏹ Остановлено");
                                    if ui.small_button("✕").clicked() {
                                        to_remove.push(i);
                                    }
                                });
                            }
                            DownloadStatus::Completed(_path) => {
                                ui.horizontal(|ui| {
                                    ui.colored_label(theme.success, "✅ Завершено");
                                    if ui
                                        .add(theme.neutral_button("📂 Открыть"))
                                        .on_hover_text("Открыть папку загрузок")
                                        .clicked()
                                    {
                                        open_folder = true;
                                    }
                                    if ui.small_button("✕").clicked() {
                                        to_remove.push(i);
                                    }
                                });
                            }
                            DownloadStatus::Failed(err) => {
                                ui.colored_label(theme.error, format!("❌ {}", err));
                                ui.horizontal(|ui| {
                                    if task.source == DownloadSource::Mp3Party {
                                        let page =
                                            format!("https://mp3party.net/music/{}", track.id);
                                        if ui
                                            .add(theme.neutral_button("🌐 Браузер"))
                                            .on_hover_text("Открыть страницу трека")
                                            .clicked()
                                        {
                                            Self::open_external_url(&page);
                                        }
                                    }
                                    if task.source == DownloadSource::DriveMusic {
                                        if let Ok(page) = Self::drivemusic_page_url(track) {
                                            if ui
                                                .add(theme.neutral_button("🌐 Браузер"))
                                                .on_hover_text("Открыть страницу трека")
                                                .clicked()
                                            {
                                                Self::open_external_url(&page);
                                            }
                                        }
                                    }
                                    if ui.small_button("✕").clicked() {
                                        to_remove.push(i);
                                    }
                                });
                            }
                        }
                    });

                    ui.add_space(6.0);
                }
            });

        if open_folder {
            self.open_downloads_folder();
        }

        for idx in to_cancel {
            self.cancel_download_task(idx);
        }

        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            self.download_tasks.remove(idx);
        }
    }

    pub fn show_playlist_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        ui.horizontal(|ui| {
            ui.label(
                theme
                    .section_title(&format!(
                        "📋 Плейлист ({} треков)",
                        self.player.playlist.len()
                    ))
                    .size(14.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme.neutral_button("🗑 Очистить")).clicked() {
                    self.player.clear_playlist();
                }
            });
        });

        ui.add_space(6.0);

        if self.player.playlist.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Плейлист пуст")
                        .size(14.0)
                        .color(theme.text_muted),
                );
                ui.label(
                    egui::RichText::new("Добавьте треки кнопкой ➕ в результатах поиска или библиотеке")
                        .size(12.0)
                        .color(theme.text_muted),
                );
            });
            return;
        }

        let scroll_height = ui.available_height().max(120.0);
        egui::ScrollArea::vertical()
            .id_salt("playlist_scroll")
            .auto_shrink([false; 2])
            .max_height(scroll_height)
            .show(ui, |ui| {
                let mut to_remove: Option<usize> = None;
                let mut to_play: Option<usize> = None;

                for (i, item) in self.player.playlist.iter().enumerate() {
                    let is_current = i == self.player.playlist_index;
                    let bg = if is_current {
                        theme.accent.gamma_multiply(0.15)
                    } else {
                        Color32::TRANSPARENT
                    };

                    Frame {
                        fill: bg,
                        rounding: Rounding::same(6.0),
                        inner_margin: Margin::symmetric(8.0, 6.0),
                        ..Default::default()
                    }
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let icon = if is_current {
                                "▶"
                            } else if item.is_url {
                                "🌐"
                            } else {
                                "🎵"
                            };
                            ui.label(icon);
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(&item.title)
                                        .strong()
                                        .color(theme.text_primary),
                                );
                                ui.label(
                                    egui::RichText::new(&item.subtitle)
                                        .size(11.0)
                                        .color(theme.text_muted),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            theme
                                                .primary_button("▶")
                                                .min_size(Vec2::new(32.0, 24.0)),
                                        )
                                        .on_hover_text("Воспроизвести")
                                        .clicked()
                                    {
                                        to_play = Some(i);
                                    }
                                    if ui.small_button("✕").clicked() {
                                        to_remove = Some(i);
                                    }
                                },
                            );
                        });
                    });
                }

                if let Some(idx) = to_play {
                    self.player.playlist_index = idx;
                    self.player.play_current();
                }
                if let Some(idx) = to_remove {
                    self.player.remove_from_playlist(idx);
                }
            });
    }
}
