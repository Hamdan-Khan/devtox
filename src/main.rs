pub mod app;
pub mod model;
pub mod tracing;
pub mod ui;
pub mod utils;

use std::io::Error;

fn main() -> Result<(), Error> {
    tracing::init_tracing();
    let mut app = app::App::new();
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
