use super::*;
use rune::{Cx, El, Env, Runtime, TextEdit, col};

#[derive(Default)]
struct Demo {
    presses: usize,
    enabled: bool,
    choice: usize,
    input: TextEdit,
}

fn view(s: &Demo, _: &mut Cx<Demo>) -> El {
    let k = theme::kit(ThemePreset::Neutral, true);
    col()
        .gap(16.0)
        .child(
            Button::new("Run")
                .action()
                .on_press(|s: &mut Demo| s.presses += 1)
                .build(k)
                .key("run"),
        )
        .child(
            TextInput::new(&s.input)
                .on_edit(|s: &mut Demo| &mut s.input)
                .trailing(
                    clear_input_button(k, "input", "Clear", |s: &mut Demo| &mut s.input)
                        .key("clear"),
                )
                .build(k)
                .key("input"),
        )
        .child(switch(
            k,
            "enabled",
            "Enabled",
            s.enabled,
            |s: &mut Demo| s.enabled = !s.enabled,
        ))
        .child(wrapping_picker(
            k,
            "choice",
            "Choice",
            &["First".into(), "Second".into()],
            s.choice,
            |s: &mut Demo, i| s.choice = i,
        ))
}

fn tap(rt: &mut Runtime<Demo>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
}

#[test]
fn components_work_with_an_independent_application_state() {
    let mut rt = Runtime::new(
        Demo::default(),
        view,
        (640.0, 480.0),
        Env {
            system_dark: true,
            scale: 1.0,
        },
    );
    rt.frame(0.0);
    tap(&mut rt, "run");
    assert_eq!(rt.state.presses, 1);
    tap(&mut rt, "enabled");
    assert!(rt.state.enabled);
    tap(&mut rt, "choice/segment1");
    assert_eq!(rt.state.choice, 1);
    tap(&mut rt, "input");
    rt.paste("independent state");
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.input.text, "independent state");
    tap(&mut rt, "clear");
    assert!(rt.state.input.text.is_empty());
    assert!(
        rt.elements()
            .iter()
            .any(|el| el.key.as_deref() == Some("input") && el.focused)
    );
}
