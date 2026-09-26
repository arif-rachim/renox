use renox::prelude::*;

struct Hello;

impl Module for Hello {
    fn name(&self) -> &'static str {
        "hello"
    }

    fn routes(&self) -> Router<AppState> {
        Router::new()
            .route("/", get(home))
            .route("/halo/{nama}", get(greet))
            .route("/boom", get(boom))
    }
}

async fn home(State(state): State<AppState>) -> Html<String> {
    Html(format!("<h1>Welcome to {}</h1>", state.config.name))
}

async fn greet(Path(nama): Path<String>) -> String {
    format!("Halo, {nama}!")
}

async fn boom() -> renox::Result<String> {
    let n: i32 = "bukan angka".parse()?;
    Ok(n.to_string())
}

fn main() -> renox::Result {
    App::new().module(Hello).run()
}
