use nekoui::{
    AppContext, Application, Color, Length, ParentElement, Render, WindowGeometry, WindowInfo,
    WindowOptions, WindowStartPosition, div, px, size, text,
};

struct HelloWorld;

impl Render for HelloWorld {
    fn render(
        &mut self,
        _window: &WindowInfo,
        _cx: &mut nekoui::Context<'_, Self>,
    ) -> impl nekoui::IntoElement {
        div()
            .w(Length::Fill)
            .h(Length::Fill)
            .p(px(24.0))
            .flex_col()
            .gap(16.0)
            .bg(Color::rgb(0xF5F3EF))
            .child(
                div()
                    .p(px(20.0))
                    .bg(Color::rgb(0x1F2937))
                    .rounded(8.0)
                    .font_family(["CaskaydiaCove Nerd Font", "Noto Sans SC"])
                    .child(
                        text(" Hello world! 我的项目不可能这么稳定！")
                            .font_size(px(32.0))
                            .text_color(Color::rgb(0xFFFFFF)),
                    ),
            )
            .child(
                div().pl(px(20.0)).child(
                    text("这是一个简单示例喵 🍥")
                        .font_family("Noto Sans SC")
                        .font_size(px(24.0))
                        .line_height(px(28.0))
                        .text_color(Color::rgb(0x111827)),
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
