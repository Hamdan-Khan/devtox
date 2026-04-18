mod app;
mod model;
mod ui;

use std::io::Error;

fn main() -> Result<(), Error> {
    let mut app = app::App::new();
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
