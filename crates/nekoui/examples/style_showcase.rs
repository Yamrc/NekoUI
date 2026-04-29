use nekoui::{
    AppContext, Application, Color, Div, Length, ParentElement, Render, WindowGeometry, WindowInfo,
    WindowOptions, WindowStartPosition, div, gradient, px, size, text,
};

struct StyleShowcase;

impl Render for StyleShowcase {
    fn render(
        &mut self,
        _window: &WindowInfo,
        _cx: &mut nekoui::Context<'_, Self>,
    ) -> impl nekoui::IntoElement {
        div()
            .w(Length::Fill)
            .h(Length::Fill)
            .p(px(26.0))
            .flex_col()
            .gap(18.0)
            .bg(Color::rgb(0xE9E1D2))
            .font_family(["Noto Sans SC"])
            .text_color(Color::rgb(0x111827))
            .child(text("Style Showcase").font_size(px(36.0)).bold())
            .child(
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(card("solid fill", Color::rgb(0x2563EB), 0xFFFFFF))
                    .child(gradient_card())
                    .child(border_card()),
            )
            .child(
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(opacity_card())
                    .child(overflow_card())
                    .child(asymmetric_radius_card()),
            )
    }
}

fn card(label: &'static str, color: Color, text_color: u32) -> Div {
    div()
        .flex_1()
        .h(px(180.0))
        .p(px(18.0))
        .rounded(24.0)
        .bg(color)
        .justify_end()
        .child(
            text(label)
                .font_size(px(22.0))
                .bold()
                .text_color(Color::rgb(text_color)),
        )
}

fn gradient_card() -> Div {
    div()
        .flex_1()
        .h(px(180.0))
        .p(px(18.0))
        .rounded(24.0)
        .bg(gradient(Color::rgb(0xF97316), Color::rgb(0xDB2777), 0.75))
        .justify_end()
        .child(
            text("linear gradient")
                .font_size(px(22.0))
                .bold()
                .text_color(Color::rgb(0xFFFFFF)),
        )
}

fn border_card() -> Div {
    div()
        .flex_1()
        .h(px(180.0))
        .p(px(18.0))
        .rounded(24.0)
        .bg(Color::rgb(0xFFFBEB))
        .border(4.0, Color::rgb(0x92400E))
        .justify_end()
        .child(text("rounded border").font_size(px(22.0)).bold())
}

fn opacity_card() -> Div {
    div()
        .flex_1()
        .h(px(220.0))
        .p(px(18.0))
        .rounded(24.0)
        .bg(Color::rgb(0x111827))
        .flex_col()
        .gap(12.0)
        .child(
            text("opacity layers")
                .font_size(px(22.0))
                .bold()
                .text_color(Color::rgb(0xFFFFFF)),
        )
        .child(layer("100%", 0x38BDF8, 1.0))
        .child(layer("65%", 0xA78BFA, 0.65))
        .child(layer("35%", 0xF472B6, 0.35))
}

fn layer(label: &'static str, color: u32, opacity: f32) -> Div {
    div()
        .w(Length::Fill)
        .h(px(36.0))
        .rounded(10.0)
        .bg(Color::rgb(color))
        .opacity(opacity)
        .justify_center()
        .items_center()
        .child(text(label).bold().text_color(Color::rgb(0xFFFFFF)))
}

fn overflow_card() -> Div {
    div()
        .flex_1()
        .h(px(220.0))
        .p(px(18.0))
        .rounded(24.0)
        .bg(Color::rgb(0xF8FAFC))
        .border(1.0, Color::rgb(0xCBD5E1))
        .flex_col()
        .gap(14.0)
        .child(text("overflow hidden").font_size(px(22.0)).bold())
        .child(
            div()
                .w(px(220.0))
                .h(px(96.0))
                .rounded(18.0)
                .overflow_hidden()
                .bg(Color::rgb(0xE0F2FE))
                .child(
                    div()
                        .w(px(320.0))
                        .h(px(120.0))
                        .m(px(20.0))
                        .rounded(28.0)
                        .bg(gradient(Color::rgb(0x06B6D4), Color::rgb(0x4338CA), 0.2)),
                ),
        )
}

fn asymmetric_radius_card() -> Div {
    div()
        .flex_1()
        .h(px(220.0))
        .p(px(18.0))
        .rounded(8.0)
        .bg(Color::rgb(0xFFFFFF))
        .border(1.0, Color::rgb(0xE5E7EB))
        .flex_col()
        .gap(16.0)
        .child(text("shape composition").font_size(px(22.0)).bold())
        .child(div().w(px(180.0)).h(px(92.0)).rounded(36.0).bg(gradient(
            Color::rgb(0x84CC16),
            Color::rgb(0x0F766E),
            1.2,
        )))
}

fn main() -> Result<(), nekoui::Error> {
    env_logger::init();
    Application::new().run(|cx: &mut AppContext<'_>| {
        cx.open_window(
            WindowOptions::new()
                .title("NekoUI Style Showcase")
                .geometry(
                    WindowGeometry::new(size(px(980.0), px(620.0)))
                        .position(WindowStartPosition::Centered),
                ),
            |_window, cx| cx.new_view(|_| StyleShowcase),
        )?;
        Ok(())
    })
}
