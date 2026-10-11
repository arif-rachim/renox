// This does not test the app: it looks at the tests you wrote in
// tests/menu_index.rs, and runs them (they are part of this file as a module).
#[path = "menu_index.rs"]
mod menu_index;

const STUDENT: &str = include_str!("menu_index.rs");

#[test]
fn the_menu_index_has_its_two_tests() {
    let tests = STUDENT.matches("#[renox::test]").count();
    assert!(
        tests >= 2,
        "tests/menu_index.rs has {tests} tests marked #[renox::test]; the challenge asks for 2"
    );
    for needle in [
        "\"/menu\"",
        "Nothing on the menu yet",
        "Cappuccino",
        "Flat white",
    ] {
        assert!(
            STUDENT.contains(needle),
            "tests/menu_index.rs never mentions {needle}"
        );
    }
}
