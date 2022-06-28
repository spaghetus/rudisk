use std::{
	fs::File,
	os::unix::prelude::MetadataExt,
	path::PathBuf,
	sync::atomic::Ordering::Relaxed,
	time::{Duration, Instant},
};

use eframe::egui::{self, Ui};
use rudisk::Search;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Opt {
	root: PathBuf,
}

pub struct App {
	search: Search,
	last_sample: (Instant, u64, u64),
	speed_record: u64,
}

impl eframe::App for App {
	fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
		ctx.request_repaint();
		egui::TopBottomPanel::top("top").show(ctx, |ui| {
			ui.horizontal(|ui| {
				egui::widgets::global_dark_light_mode_switch(ui);
				ui.separator();
				// Recalculate files/sec if applicable
				if (Instant::now() - self.last_sample.0) > Duration::from_secs(1) {
					let count = self
						.search
						.searched
						.load(std::sync::atomic::Ordering::Relaxed);
					let rate = count - self.last_sample.1;
					if rate > self.speed_record {
						self.speed_record = rate
					}
					self.last_sample = (Instant::now(), count, rate);
				}
				ui.label(format!(
					"{}/s, peak {}/s",
					self.last_sample.2, self.speed_record
				));
				ui.separator();
				ui.label(format!(
					"{} searched, {} total size",
					self.search
						.searched
						.load(std::sync::atomic::Ordering::Relaxed),
					pretty_bytes::converter::convert(
						self.search.size.load(std::sync::atomic::Ordering::Relaxed) as f64
					)
				));
				ui.separator();
				if self.search.is_finished() {
					ui.label("Rudisk Done");
				} else {
					ui.label("Rudisk Working");
				}
			})
		});
		egui::CentralPanel::default().show(ctx, |ui| {
			egui::ScrollArea::new([false, true])
				.auto_shrink([false, false])
				.show(ui, |ui| {
					fn visitor(path: &PathBuf, parent_size: u64, ui: &mut Ui, search: &Search) {
						let file = if let Ok(f) = File::open(path) {
							f
						} else {
							return;
						};
						let name = path
							.components()
							.last()
							.unwrap()
							.as_os_str()
							.to_string_lossy();
						let meta = file.metadata().unwrap();
						let size = search
							.sizes
							.read()
							.unwrap()
							.get(&meta.ino())
							.map(|v| v.load(Relaxed));
						if meta.is_dir() {
							egui::CollapsingHeader::new(format!(
								"{}, {}, {}%",
								name,
								size.map(|v| pretty_bytes::converter::convert(v as f64))
									.unwrap_or("???".to_string()),
								size.map(|size| (size as f64 / parent_size as f64) * 100.0)
									.map(|v| v.to_string())
									.unwrap_or("???".to_string())
							))
							.id_source(path)
							.show(ui, |ui| {
								let mut children = std::fs::read_dir(path)
									.unwrap()
									.flatten()
									.collect::<Vec<_>>();
								children.sort_by(|a, b| {
									let a = search
										.sizes
										.read()
										.unwrap()
										.get(&a.metadata().unwrap().ino())
										.map(|v| v.load(Relaxed))
										.unwrap_or(0);
									let b = search
										.sizes
										.read()
										.unwrap()
										.get(&b.metadata().unwrap().ino())
										.map(|v| v.load(Relaxed))
										.unwrap_or(0);
									b.cmp(&a)
								});
								for i in children {
									visitor(&i.path(), size.unwrap_or(0), ui, search)
								}
							});
						} else {
							ui.label(format!(
								"{}, {}, {}%",
								name,
								size.map(|v| pretty_bytes::converter::convert(v as f64))
									.unwrap_or("???".to_string()),
								size.map(|size| (size / parent_size) * 100)
									.map(|v| v.to_string())
									.unwrap_or("???".to_string())
							));
						}
					}
					visitor(
						&self.search.root,
						self.search.size.load(Relaxed),
						ui,
						&self.search,
					)
				})
		});
	}
}

fn main() {
	let args = Opt::from_args();
	let mut search = Search::new(args.root);
	search.go().unwrap();
	eframe::run_native(
		"Rudisk",
		Default::default(),
		Box::new(|_| {
			Box::new(App {
				search,
				last_sample: (Instant::now(), 0, 0),
				speed_record: 0,
			})
		}),
	)
}
