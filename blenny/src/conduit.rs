// blenny/src/conduit.rs
use std::io;
use tera::{Context as TeraContext, Tera};

pub struct Conduit {
    engine: Tera,
}

impl Conduit {
    pub fn hot_reload(template_dir: &str) -> Result<Self, io::Error> {
        let engine = Tera::new(&format!("{}/**/*.tera", template_dir))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        println!(
            "Tera templates loaded: {:?}",
            engine.get_template_names().collect::<Vec<_>>()
        );
        Ok(Conduit { engine })
    }

    pub fn frozen() -> Result<Self, io::Error> {
        Ok(Conduit {
            engine: Tera::default(),
        })
    }

    pub fn render(&self, template: &str, ctx: &TeraContext) -> Result<String, tera::Error> {
        let template_name = if template.ends_with(".tera") {
            template.to_string()
        } else {
            format!("{}.tera", template)
        };
        self.engine.render(&template_name, ctx)
    }
}
