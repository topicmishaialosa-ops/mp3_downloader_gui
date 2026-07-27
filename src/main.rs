#![cfg_attr(windows, windows_subsystem = "windows")]

mod batch;
mod library;
mod player;

mod types;
mod theme;
mod parsing;
mod downloader;
mod app;
mod panels;

use eframe::egui;
use types::LinkParserApp;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 680.0])
            .with_min_inner_size([600.0, 450.0])
            .with_title("🎵 MP3Party Parser — загрузка треков"),
        ..Default::default()
    };

    eframe::run_native(
        "mp3party_link_parser",
        options,
        Box::new(|_cc| Ok(Box::new(LinkParserApp::default()))),
    )
}
