use std::time::Duration;

pub enum Diagnostic {
    ModuleBuildSuccess(ModuleBuildSuccess),
}

pub struct ModuleBuildSuccess {
    pub module_id: String,
    pub duration: Duration,
}

pub struct Diagnostics {
    diagnostics: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn print(&self) {
        for diagnostic in &self.diagnostics {
            match diagnostic {
                Diagnostic::ModuleBuildSuccess(mds) => {
                    println!(
                        "Module {} built in {}ms",
                        &mds.module_id,
                        &mds.duration.as_millis()
                    )
                }
            }
        }
    }
}
