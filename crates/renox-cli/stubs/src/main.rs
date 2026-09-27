mod app;

fn main() -> renox::Result {
    renox::App::new()
        .migrations(renox::migrations!())
        .module(app::home::Home)
        .run()
}
