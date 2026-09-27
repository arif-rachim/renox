mod app;

fn main() -> renox::Result {
    renox::App::new()
        .migrations(renox::migrations!())
        .module(renox::auth::Auth::new())
        .module(app::home::Home)
        .run()
}
