use notify::{EventKind, Watcher};
use std::io;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tera::{Context as TeraContext, Tera};

use crate::embedded::EmbeddedTemplates;

pub struct Conduit {
    engine: Arc<RwLock<Tera>>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl Conduit {
    /// Hot‑reload from a directory (development).
    /// Starts a background watcher that reloads templates on change.
    pub fn hot_reload(template_dir: &str) -> Result<Self, io::Error> {
        // Initial engine load
        let engine = Tera::new(&format!("{}/**/*.tera", template_dir))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        println!(
            "Tera templates loaded: {:?}",
            engine.get_template_names().collect::<Vec<_>>()
        );

        let engine = Arc::new(RwLock::new(engine));

        // Set up file watcher
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let dir = std::path::PathBuf::from(template_dir);
        watcher
            .watch(&dir, notify::RecursiveMode::Recursive)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Background task with debounced reload
        let engine_clone = engine.clone();
        tokio::spawn(async move {
            let mut debounce: Option<tokio::task::JoinHandle<()>> = None;
            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        match event {
                            Ok(event) if matches!(event.kind, EventKind::Modify(_)) => {
                                if debounce.is_none() {
                                    let eng = engine_clone.clone();
                                    debounce = Some(tokio::spawn(async move {
                                        tokio::time::sleep(Duration::from_millis(50)).await;
                                        if let Ok(mut engine) = eng.write() {
                                            if let Err(e) = engine.full_reload() {
                                                eprintln!("Template reload error: {e}");
                                            } else {
                                                println!("Templates reloaded.");
                                            }
                                        }
                                    }));
                                }
                            }
                            Err(e) => eprintln!("Watch error: {e}"),
                            _ => {}
                        }
                    }
                }
                // Clear the debounce handle when the reload task is done.
                if let Some(ref handle) = debounce {
                    if handle.is_finished() {
                        debounce = None;
                    }
                }
            }
        });

        Ok(Conduit {
            engine,
            _watcher: Some(watcher),
        })
    }

    /// Frozen mode – embed templates from binary (production).
    pub fn frozen() -> Result<Self, io::Error> {
        let mut tera = Tera::default();
        for file in EmbeddedTemplates::iter() {
            let filename = file.as_ref();
            if let Some(content) = EmbeddedTemplates::get(filename) {
                let content_str = std::str::from_utf8(content.data.as_ref())
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                tera.add_raw_template(filename, content_str)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            }
        }
        println!(
            "Frozen templates loaded: {:?}",
            tera.get_template_names().collect::<Vec<_>>()
        );
        Ok(Conduit {
            engine: Arc::new(RwLock::new(tera)),
            _watcher: None,
        })
    }

    pub fn render(&self, template: &str, ctx: &TeraContext) -> Result<String, tera::Error> {
        let engine = self.engine.read().unwrap_or_else(|e| e.into_inner());
        let template_name = if template.ends_with(".tera") {
            template.to_string()
        } else {
            format!("{}.tera", template)
        };
        engine.render(&template_name, ctx)
    }
}
