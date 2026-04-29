use nekoui::{
    AppContext, Application, Color, Length, ParentElement, Render, WindowGeometry, WindowInfo,
    WindowOptions, WindowStartPosition, div, point, px, size, text,
};

struct WindowPanel {
    title: &'static str,
    subtitle: &'static str,
    accent: u32,
}

struct PanelSpec {
    title: &'static str,
    subtitle: &'static str,
    accent: u32,
    size: (f32, f32),
    position: (f32, f32),
    titlebar: bool,
}

impl Render for WindowPanel {
    fn render(
        &mut self,
        window: &WindowInfo,
        _cx: &mut nekoui::Context<'_, Self>,
    ) -> impl nekoui::IntoElement {
        div()
            .w(Length::Fill)
            .h(Length::Fill)
            .p(px(22.0))
            .flex_col()
            .gap(14.0)
            .bg(Color::rgb(0xF8F5EE))
            .font_family(["Noto Sans SC"])
            .child(
                div()
                    .h(px(10.0))
                    .rounded(99.0)
                    .bg(Color::rgb(self.accent)),
            )
            .child(text(self.title).font_size(px(30.0)).bold())
            .child(
                text(self.subtitle)
                    .font_size(px(18.0))
                    .line_height(px(24.0))
                    .text_color(Color::rgb(0x374151)),
            )
            .child(
                div()
                    .p(px(14.0))
                    .rounded(16.0)
                    .bg(Color::rgb(0xFFFFFF))
                    .border(1.0, Color::rgb(0xE5E7EB))
                    .child(
                        text(format!(
                            "logical: {} x {} px",
                            window.content_size().width,
                            window.content_size().height
                        ))
                        .font_size(px(16.0))
                        .text_color(Color::rgb(0x111827)),
                    ),
            )
            .child(
                text("Each window owns an independent retained tree, compiled scene, and render state.")
                    .font_size(px(15.0))
                    .line_height(px(21.0))
                    .text_color(Color::rgb(0x6B7280)),
            )
    }
}

fn main() -> Result<(), nekoui::Error> {
    env_logger::init();
    Application::new().run(|cx: &mut AppContext<'_>| {
        open_panel(
            cx,
            PanelSpec {
                title: "Main Window",
                subtitle: "Primary application surface.",
                accent: 0x2563EB,
                size: (760.0, 460.0),
                position: (80.0, 80.0),
                titlebar: false,
            },
        )?;
        open_panel(
            cx,
            PanelSpec {
                title: "Inspector",
                subtitle: "Secondary window with different size and content.",
                accent: 0xE11D48,
                size: (420.0, 360.0),
                position: (900.0, 120.0),
                titlebar: true,
            },
        )?;
        open_panel(
            cx,
            PanelSpec {
                title: "Log",
                subtitle: "A third independent surface.",
                accent: 0x059669,
                size: (520.0, 300.0),
                position: (220.0, 620.0),
                titlebar: true,
            },
        )?;
        Ok(())
    })
}

fn open_panel(cx: &mut AppContext<'_>, spec: PanelSpec) -> Result<(), nekoui::Error> {
    cx.open_window(
        WindowOptions::new()
            .title(spec.title)
            .geometry(
                WindowGeometry::new(size(px(spec.size.0), px(spec.size.1))).position(
                    WindowStartPosition::Absolute(point(px(spec.position.0), px(spec.position.1))),
                ),
            )
            .show_titlebar(spec.titlebar),
        move |_window, cx| {
            cx.new_view(move |_| WindowPanel {
                title: spec.title,
                subtitle: spec.subtitle,
                accent: spec.accent,
            })
        },
    )?;
    Ok(())
}
