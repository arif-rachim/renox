mod app;

fn main() -> renox::Result {
    renox::App::new().module(app::home::Home).run()
}
