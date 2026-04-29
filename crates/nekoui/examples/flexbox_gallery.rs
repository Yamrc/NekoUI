use nekoui::{
    AppContext, Application, Color, Div, FlexWrap, IntoElement, JustifyContent, Length,
    ParentElement, Render, WindowGeometry, WindowInfo, WindowOptions, WindowStartPosition, div, px,
    size, text,
};

struct FlexboxGallery;

impl Render for FlexboxGallery {
    fn render(
        &mut self,
        _window: &WindowInfo,
        _cx: &mut nekoui::Context<'_, Self>,
    ) -> impl IntoElement {
        div()
            .w(Length::Fill)
            .h(Length::Fill)
            .p(px(24.0))
            .flex_col()
            .gap(18.0)
            .bg(Color::rgb(0xF4F0E8))
            .font_family(["Noto Sans SC"])
            .text_color(Color::rgb(0x18212F))
            .child(
                text("Flexbox Gallery")
                    .font_size(px(34.0))
                    .bold()
                    .line_height(px(38.0)),
            )
            .child(section(
                "row / column / wrap",
                div()
                    .flex_row()
                    .flex_wrap(FlexWrap::Wrap)
                    .gap(10.0)
                    .child(tile("A", 0xF97316, 92.0))
                    .child(tile("B", 0x14B8A6, 132.0))
                    .child(tile("C", 0x6366F1, 112.0))
                    .child(tile("D", 0xE11D48, 152.0))
                    .child(tile("E", 0x84CC16, 102.0)),
            ))
            .child(section(
                "gap / justify-content / align-items",
                div()
                    .h(px(120.0))
                    .p(px(14.0))
                    .rounded(18.0)
                    .bg(Color::rgb(0x101827))
                    .flex_row()
                    .justify(JustifyContent::SpaceBetween)
                    .items_center()
                    .child(small_box("start", 0xF8FAFC))
                    .child(small_box("center", 0xF8FAFC))
                    .child(small_box("end", 0xF8FAFC)),
            ))
            .child(section(
                "align-self",
                div()
                    .h(px(128.0))
                    .p(px(14.0))
                    .rounded(18.0)
                    .bg(Color::rgb(0xE7E0D2))
                    .flex_row()
                    .gap(12.0)
                    .items_start()
                    .child(aligned_box("self-start", 0x0EA5E9).self_start())
                    .child(aligned_box("self-center", 0x22C55E).self_center())
                    .child(aligned_box("self-end", 0xF59E0B).self_end()),
            ))
            .child(section(
                "flex-grow / shrink / basis",
                div()
                    .h(px(86.0))
                    .p(px(12.0))
                    .rounded(18.0)
                    .bg(Color::rgb(0x111827))
                    .flex_row()
                    .gap(8.0)
                    .items_center()
                    .child(flex_piece("grow: 2", 0x38BDF8, 2.0))
                    .child(flex_piece("grow: 1", 0xA78BFA, 1.0))
                    .child(flex_piece("basis 180", 0xF472B6, 0.0).flex_basis(px(180.0))),
            ))
    }
}

fn section(title: &'static str, content: impl IntoElement) -> Div {
    div()
        .p(px(16.0))
        .rounded(22.0)
        .border(1.0, Color::rgb(0xDDD4C4))
        .bg(Color::rgb(0xFFFCF5))
        .flex_col()
        .gap(12.0)
        .child(text(title).font_size(px(18.0)).bold())
        .child(content)
}

fn tile(label: &'static str, color: u32, width: f32) -> Div {
    div()
        .w(px(width))
        .h(px(64.0))
        .rounded(14.0)
        .bg(Color::rgb(color))
        .justify_center()
        .items_center()
        .child(
            text(label)
                .font_size(px(22.0))
                .bold()
                .text_color(Color::rgb(0xFFFFFF)),
        )
}

fn small_box(label: &'static str, color: u32) -> Div {
    div()
        .w(px(128.0))
        .h(px(52.0))
        .rounded(12.0)
        .bg(Color::rgb(color))
        .justify_center()
        .items_center()
        .child(text(label).bold())
}

fn aligned_box(label: &'static str, color: u32) -> Div {
    div()
        .w(px(150.0))
        .h(px(44.0))
        .rounded(12.0)
        .bg(Color::rgb(color))
        .justify_center()
        .items_center()
        .child(
            text(label)
                .font_size(px(14.0))
                .bold()
                .text_color(Color::rgb(0xFFFFFF)),
        )
}

fn flex_piece(label: &'static str, color: u32, grow: f32) -> Div {
    div()
        .flex_grow(grow)
        .h(px(52.0))
        .rounded(12.0)
        .bg(Color::rgb(color))
        .justify_center()
        .items_center()
        .child(text(label).bold().text_color(Color::rgb(0x0F172A)))
}

fn main() -> Result<(), nekoui::Error> {
    env_logger::init();
    Application::new().run(|cx: &mut AppContext<'_>| {
        cx.open_window(
            WindowOptions::new()
                .title("NekoUI Flexbox Gallery")
                .geometry(
                    WindowGeometry::new(size(px(980.0), px(760.0)))
                        .position(WindowStartPosition::Centered),
                ),
            |_window, cx| cx.new_view(|_| FlexboxGallery),
        )?;
        Ok(())
    })
}
