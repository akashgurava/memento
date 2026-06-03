use std::{collections::HashMap, fmt, path::PathBuf};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::bus::Consumer;
use crate::error::MementoError;
use crate::scan::{FileKind, ScanFile, ScanMessage};

#[derive(Default, Clone, Debug)]
struct ScanMetric {
    img_count: u64,
    img_size: u64,
    vid_count: u64,
    vid_size: u64,
    other_count: u64,
    other_size: u64,
}

impl ScanMetric {
    fn add_file(&mut self, file: &ScanFile) {
        match file.kind() {
            FileKind::Image => {
                self.img_count += 1;
                self.img_size += file.metadata().len();
            }
            FileKind::Video => {
                self.vid_count += 1;
                self.vid_size += file.metadata().len();
            }
            FileKind::Other => {
                self.other_count += 1;
                self.other_size += file.metadata().len();
            }
        }
    }

    fn format_bytes(bytes: u64) -> String {
        const KIB: u64 = 1024;
        const MIB: u64 = KIB * 1024;
        const GIB: u64 = MIB * 1024;

        if bytes >= GIB {
            format!("{:.2} GiB", bytes as f64 / GIB as f64)
        } else if bytes >= MIB {
            format!("{:.2} MiB", bytes as f64 / MIB as f64)
        } else if bytes >= KIB {
            format!("{:.2} KiB", bytes as f64 / KIB as f64)
        } else {
            format!("{} B", bytes)
        }
    }
}

impl fmt::Display for ScanMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Img: {} ({}) | Vid: {} ({}) | Oth: {} ({})",
            self.img_count,
            Self::format_bytes(self.img_size),
            self.vid_count,
            Self::format_bytes(self.vid_size),
            self.other_count,
            Self::format_bytes(self.other_size)
        )
    }
}

pub struct ScanCliConsumer {
    progress: MultiProgress,
    folder_style: ProgressStyle,
    global_bar: ProgressBar,
    global_metric: ScanMetric,
    folder_bars: HashMap<PathBuf, ProgressBar>,
    folder_metrics: HashMap<PathBuf, ScanMetric>,
}

impl ScanCliConsumer {
    pub fn new(_roots: &[PathBuf]) -> Result<Self, MementoError> {
        let progress = MultiProgress::new();
        let global_style = ProgressStyle::default_spinner()
            .template("{spinner:.green} [TOTAL] {msg}")
            .map_err(|e| MementoError::TemplateError(e.to_string()))?;
        let global_bar = progress.add(ProgressBar::new_spinner());
        global_bar.set_style(global_style);

        let folder_style = ProgressStyle::default_spinner()
            .template("{spinner:.cyan} [{prefix}] {msg}")
            .map_err(|e| MementoError::TemplateError(e.to_string()))?;

        Ok(Self {
            progress,
            folder_style,
            global_bar,
            global_metric: ScanMetric::default(),
            folder_bars: HashMap::new(),
            folder_metrics: HashMap::new(),
        })
    }
}

impl Consumer for ScanCliConsumer {
    type Message = ScanMessage;

    fn consume(&mut self, message: &Self::Message) -> Result<(), MementoError> {
        match message {
            ScanMessage::Error { error } => {
                tracing::error!("{error}");
            }
            ScanMessage::RootStart { path } => {
                let pb = self.progress.add(ProgressBar::new_spinner());
                pb.set_style(self.folder_style.clone());
                pb.set_prefix(path.to_string_lossy().into_owned());
                self.folder_bars.insert(path.clone(), pb);
                self.folder_metrics
                    .insert(path.clone(), ScanMetric::default());
            }
            ScanMessage::Dir { .. } => {
                self.global_bar.tick();
            }
            ScanMessage::File { file } => {
                self.global_metric.add_file(file);
                self.global_bar.set_message(self.global_metric.to_string());

                if let Some(metrics) = self.folder_metrics.get_mut(file.root().as_ref()) {
                    metrics.add_file(file);
                    if let Some(pb) = self.folder_bars.get(file.root().as_ref()) {
                        pb.set_message(metrics.to_string());
                    }
                }
            }
            ScanMessage::RootDone { path } => {
                if let Some(pb) = self.folder_bars.remove(path) {
                    if let Some(metrics) = self.folder_metrics.remove(path) {
                        pb.finish_with_message(format!("[DONE] {metrics}"));
                    }
                }
            }
            ScanMessage::Done => {
                self.global_bar
                    .finish_with_message(format!("[DONE] {}", self.global_metric));
            }
            ScanMessage::Cancelled => {}
        }
        Ok(())
    }
}
