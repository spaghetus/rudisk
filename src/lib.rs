use rayon::prelude::*;
use std::{
	collections::BTreeMap,
	fs::File,
	os::unix::prelude::MetadataExt,
	path::PathBuf,
	sync::{
		atomic::{AtomicU64, Ordering::Relaxed},
		mpsc::Sender,
		Arc, RwLock,
	},
	thread::JoinHandle,
};
use walkdir::{DirEntry, WalkDir};

#[derive(Default)]
#[non_exhaustive]
pub struct Search {
	pub sizes: Arc<RwLock<BTreeMap<u64, AtomicU64>>>,
	pub thread: Option<JoinHandle<()>>,
	pub root: PathBuf,
	pub searched: Arc<AtomicU64>,
	pub size: Arc<AtomicU64>,
}

impl Search {
	pub fn new(root: PathBuf) -> Search {
		Search {
			root,
			..Default::default()
		}
	}
	pub fn go(&mut self) -> Result<(), &'static str> {
		if self.thread.is_some() {
			return Err("Already running");
		}
		let root = self.root.clone();
		let sizes = self.sizes.clone();
		let searched = self.searched.clone();
		let size = self.size.clone();
		self.thread = Some(std::thread::spawn(move || {
			WalkDir::new(root.clone())
				.same_file_system(true)
				.into_iter()
				.par_bridge()
				.flatten()
				.filter(|v| v.metadata().unwrap().is_file())
				.for_each(move |v| {
					let meta = v.metadata().unwrap();
					let id = meta.ino();
					let len = meta.len();
					let path = v.path();
					searched.fetch_add(1, Relaxed);
					size.fetch_add(len, Relaxed);
					sizes.write().unwrap().insert(id, AtomicU64::new(len));
					for i in path.ancestors() {
						let ancestor = if let Ok(a) = File::open(i) { a } else { return };
						let meta = ancestor.metadata().unwrap();
						// Ensure that the key exists
						sizes
							.write()
							.unwrap()
							.entry(meta.ino())
							.or_insert(AtomicU64::new(0));
						// Add the size
						sizes
							.read()
							.unwrap()
							.get(&meta.ino())
							.unwrap()
							.fetch_add(len, Relaxed);
					}
				});
		}));
		Ok(())
	}
	pub fn is_finished(&mut self) -> bool {
		match &self.thread {
			Some(t) if t.is_finished() => true,
			_ => false,
		}
	}
}
