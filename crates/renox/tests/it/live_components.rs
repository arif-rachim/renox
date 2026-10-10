//! Registering live components.

use renox::live_component::LiveContext;
use renox::prelude::*;

#[derive(serde::Serialize, serde::Deserialize)]
struct Counter {
    count: i64,
}

impl LiveComponent for Counter {
    const NAME: &'static str = "counter";
    const VIEW: &'static str = "live/counter.html";

    async fn call(
        &mut self,
        action: &str,
        _args: Vec<serde_json::Value>,
        _ctx: &mut LiveContext,
    ) -> Result {
        match action {
            "increment" => {
                self.count += 1;
                Ok(())
            }
            _ => Err(Error::NotFound),
        }
    }
}

#[renox::test]
async fn a_component_registers_through_the_app() {
    let app = App::with_config(Config::default()).live_component::<Counter>();
    assert!(app.boot().await.is_ok());
}

#[renox::test]
async fn a_duplicate_component_is_a_boot_error() {
    let err = App::with_config(Config::default())
        .live_component::<Counter>()
        .live_component::<Counter>()
        .boot()
        .await
        .err()
        .unwrap();
    assert!(
        format!("{err:?}").contains("live component `counter` is registered twice"),
        "{err:?}"
    );
}
