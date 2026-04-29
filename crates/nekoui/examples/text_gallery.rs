use nekoui::{
    AppContext, Application, Color, Div, FontWeight, IntoElement, Length, ParentElement, Render,
    WindowGeometry, WindowInfo, WindowOptions, WindowStartPosition, div, px, size, text,
};

struct TextGallery;

impl Render for TextGallery {
    fn render(
        &mut self,
        _window: &WindowInfo,
        _cx: &mut nekoui::Context<'_, Self>,
    ) -> impl IntoElement {
        div()
            .w(Length::Fill)
            .h(Length::Fill)
            .p(px(26.0))
            .flex_col()
            .gap(18.0)
            .bg(Color::rgb(0x111827))
            .font_family(["Noto Sans SC", "Segoe UI Emoji"])
            .text_color(Color::rgb(0xF9FAFB))
            .child(
                text("Text Gallery")
                    .font_size(px(36.0))
                    .bold()
                    .line_height(px(40.0)),
            )
            .child(row(
                "fallback",
                "Latin AaBbCc  中文排版  日本語  한국어  عربى ✅❇️🔴🟠🟡🟢🔵🟣🟤⚫🐱🚀",
            ))
            .child(
                div()
                    .flex_row()
                    .gap(12.0)
                    .child(weight_card("Normal", FontWeight::Normal))
                    .child(weight_card("Medium", FontWeight::Medium))
                    .child(weight_card("Semibold", FontWeight::Semibold))
                    .child(weight_card("Bold", FontWeight::Bold)),
            )
            .child(
                div()
                    .flex_row()
                    .gap(12.0)
                    .items_end()
                    .child(size_sample("12px", 12.0))
                    .child(size_sample("18px", 18.0))
                    .child(size_sample("28px", 28.0))
                    .child(size_sample("42px", 42.0)),
            )
            .child(
                div()
                    .flex_row()
                    .gap(12.0)
                    .child(alignment_card("left", 0x1F2937, AlignMode::Left))
                    .child(alignment_card("center", 0x243B53, AlignMode::Center))
                    .child(alignment_card("right", 0x334155, AlignMode::Right)),
            )
            .child(
                div()
                    .p(px(16.0))
                    .rounded(18.0)
                    .bg(Color::rgb(0xF9FAFB))
                    .flex_col()
                    .gap(8.0)
                    .child(
                        text("nowrap + ellipsis")
                            .font_size(px(14.0))
                            .bold()
                            .text_color(Color::rgb(0x6B7280)),
                    )
                    .child(
                        text("This is a deliberately long single-line text run that should be clipped with an ellipsis when the box is narrow.")
                            .w(px(520.0))
                            .whitespace_nowrap()
                            .truncate()
                            .font_size(px(20.0))
                            .text_color(Color::rgb(0x111827)),
                    ),
            )
    }
}

enum AlignMode {
    Left,
    Center,
    Right,
}

fn row(label: &'static str, sample: &'static str) -> Div {
    div()
        .p(px(16.0))
        .rounded(18.0)
        .bg(Color::rgb(0x1F2937))
        .flex_col()
        .gap(8.0)
        .child(
            text(label)
                .font_size(px(14.0))
                .bold()
                .text_color(Color::rgb(0x93C5FD)),
        )
        .child(text(sample).font_size(px(23.0)).line_height(px(30.0)))
}

fn weight_card(label: &'static str, weight: FontWeight) -> Div {
    div()
        .flex_1()
        .h(px(86.0))
        .p(px(12.0))
        .rounded(16.0)
        .bg(Color::rgb(0xF8FAFC))
        .justify_center()
        .items_center()
        .child(
            text(label)
                .font_weight(weight)
                .font_size(px(22.0))
                .text_color(Color::rgb(0x111827)),
        )
}

fn size_sample(label: &'static str, size: f32) -> Div {
    div()
        .flex_1()
        .h(px(88.0))
        .rounded(16.0)
        .bg(Color::rgb(0x1E293B))
        .justify_center()
        .items_center()
        .child(text(label).font_size(px(size)).bold())
}

fn alignment_card(label: &'static str, color: u32, mode: AlignMode) -> Div {
    let sample = text("Aligned text")
        .w(Length::Fill)
        .font_size(px(18.0))
        .text_color(Color::rgb(0xFFFFFF));
    let sample = match mode {
        AlignMode::Left => sample.text_left(),
        AlignMode::Center => sample.text_center(),
        AlignMode::Right => sample.text_right(),
    };

    div()
        .flex_1()
        .p(px(14.0))
        .rounded(16.0)
        .bg(Color::rgb(color))
        .flex_col()
        .gap(10.0)
        .child(
            text(label)
                .font_size(px(14.0))
                .text_color(Color::rgb(0xCBD5E1)),
        )
        .child(sample)
}

fn main() -> Result<(), nekoui::Error> {
    env_logger::init();
    Application::new().run(|cx: &mut AppContext<'_>| {
        cx.open_window(
            WindowOptions::new().title("NekoUI Text Gallery").geometry(
                WindowGeometry::new(size(px(960.0), px(680.0)))
                    .position(WindowStartPosition::Centered),
            ),
            |_window, cx| cx.new_view(|_| TextGallery),
        )?;
        Ok(())
    })
}
