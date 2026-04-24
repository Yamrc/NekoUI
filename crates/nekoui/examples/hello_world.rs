use nekoui::{
    AppContext, Application, Color, Direction, EdgeInsets, Length, ParentElement, Render,
    WindowGeometry, WindowInfo, WindowOptions, WindowStartPosition, div, px, size, text,
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
            .padding(EdgeInsets::all(24.0))
            .direction(Direction::Column)
            .gap(16.0)
            .bg(Color::rgb(0xF5F3EF))
            .child(
                div()
                    .padding(EdgeInsets::all(20.0))
                    .bg(Color::rgb(0x1F2937))
                    .corner_radius(8.0)
                    .child(
                        text("Hello world!")
                            .font_size(32.0)
                            .color(Color::rgb(0xFFFFFF)),
                    ),
            )
            .child(
                div()
                    .padding(EdgeInsets {
                        top: 0.0,
                        right: 0.0,
                        bottom: 0.0,
                        left: 20.0,
                    })
                    .child(
                        text("这是一个简单示例喵 🍥")
                            .font_size(24.0)
                            .line_height(28.0)
                            .color(Color::rgb(0x111827)),
                    ),
            )
    }
}

fn main() -> Result<(), nekoui::Error> {
    env_logger::init();
    Application::new().run(|cx: &mut AppContext<'_>| {
        cx.open_window(
            WindowOptions::new().title("NekoUI").geometry(
                WindowGeometry::new(size(px(800.0), px(500.0)))
                    .position(WindowStartPosition::Centered),
            ),
            |_window, cx| cx.new_view(|_| HelloWorld),
        )?;
        Ok(())
    })
}
