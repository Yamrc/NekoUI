use nekoui::{
    AppContext, Application, Color, Direction, Length, ParentElement, Render, WindowGeometry,
    WindowInfo, WindowOptions, WindowStartPosition, div, px, size, text,
};

struct HelloWorld;

impl Render for HelloWorld {
    fn render(
        &mut self,
        _window: &WindowInfo,
        _cx: &mut nekoui::Context<'_, Self>,
    ) -> impl nekoui::IntoElement {
        div()
            .width(Length::Fill)
            .height(Length::Fill)
            .direction(Direction::Column)
            .bg(Color::rgb(0xF5F3EF))
            .child(
                div()
                    .bg(Color::rgb(0x000000))
                    .width(Length::Fill)
                    .height(px(50.))
                    .window_drag_area()
                    .justify(nekoui::JustifyContent::End)
                    .children((
                        div()
                            .width(px(50.))
                            .height(px(50.0))
                            .window_minimize_button()
                            .bg(Color::rgb(0xffffff))
                            .justify(nekoui::JustifyContent::Center)
                            .align_items(nekoui::AlignItems::Center)
                            .child(text("-")),
                        div()
                            .width(px(50.))
                            .height(px(50.0))
                            .window_maximize_button()
                            .bg(Color::rgb(0xffffff))
                            .justify(nekoui::JustifyContent::Center)
                            .align_items(nekoui::AlignItems::Center)
                            .child(text("口")),
                        div()
                            .width(px(50.))
                            .height(px(50.0))
                            .window_close_button()
                            .bg(Color::rgb(0xffffff))
                            .justify(nekoui::JustifyContent::Center)
                            .align_items(nekoui::AlignItems::Center)
                            .child(text("x")),
                    )),
            )
    }
}

fn main() -> Result<(), nekoui::Error> {
    env_logger::init();
    Application::new().run(|cx: &mut AppContext<'_>| {
        cx.open_window(
            WindowOptions::new()
                .title("NOTITLEBAR")
                .show_titlebar(false)
                .geometry(
                    WindowGeometry::new(size(px(800.0), px(500.0)))
                        .position(WindowStartPosition::Default),
                ),
            |_window, cx| cx.new_view(|_| HelloWorld),
        )?;
        Ok(())
    })
}
