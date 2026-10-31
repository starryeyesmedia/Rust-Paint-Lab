//! Locally bundled Google Material Symbols Rounded assets, rendered from a
//! cached monochrome texture. The matching SVG sources stay unchanged beside
//! the generated, offline PNG masks used by egui.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Brush,
    Smudge,
    ColorMixer,
    Eraser,
    Line,
    Path,
    QuickShape,
    Gradient,
    Rectangle,
    Ellipse,
    RectangleSelection,
    EllipseSelection,
    Lasso,
    Eyedropper,
    Deselect,
    Fill,
    Undo,
    Redo,
    Open,
    Save,
    Layers,
    Color,
    Symmetry,
    Settings,
    Reference,
    DragHandle,
    ChevronLeft,
    ChevronRight,
    Eye,
    EyeOff,
    Lock,
    Unlock,
    More,
    EditAnchors,
    AddBox,
    Check,
    Close,
    ExpandMore,
}

fn paint(painter: &egui::Painter, bounds: egui::Rect, icon: Icon, color: egui::Color32) {
    let texture = texture(painter.ctx(), icon);
    painter.image(
        texture.id(),
        bounds,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        color,
    );
}

pub fn draw_symbol(painter: &egui::Painter, bounds: egui::Rect, icon: Icon, color: egui::Color32) {
    paint(painter, bounds, icon, color);
}

pub fn group_button(
    ui: &mut egui::Ui,
    icon: Icon,
    family: &str,
    current: &str,
    count: usize,
    expanded: bool,
) -> egui::Response {
    let state = if expanded { "menu open" } else { "menu closed" };
    let label = format!("{family}, current tool {current}, {count} tools, {state}");
    let response = ui.add_sized(
        [44.0, 44.0],
        egui::Button::new(
            egui::RichText::new(&label)
                .size(1.0)
                .color(egui::Color32::TRANSPARENT),
        )
        .selected(expanded),
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
    let color = if expanded {
        ui.visuals().selection.stroke.color
    } else if response.hovered() {
        ui.visuals().widgets.hovered.fg_stroke.color
    } else {
        ui.visuals().text_color()
    };
    paint(ui.painter(), response.rect.shrink(10.0), icon, color);
    let badge = egui::Rect::from_center_size(
        response.rect.right_bottom() - egui::vec2(8.0, 8.0),
        egui::vec2(16.0, 16.0),
    );
    paint(ui.painter(), badge, Icon::More, color);
    response.on_hover_text(format!("{family} · {current} · {count} tools"))
}

pub fn menu_choice(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    shortcut: Option<&str>,
    selected: bool,
) -> egui::Response {
    let width = ui.available_width().max(220.0);
    let response = ui.add_sized(
        [width, 44.0],
        egui::Button::selectable(selected, format!("    {label}")),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, label)
    });
    let color = if selected {
        ui.visuals().selection.stroke.color
    } else if response.hovered() {
        ui.visuals().widgets.hovered.fg_stroke.color
    } else {
        ui.visuals().text_color()
    };
    paint(
        ui.painter(),
        egui::Rect::from_center_size(
            egui::pos2(response.rect.left() + 22.0, response.rect.center().y),
            egui::vec2(20.0, 20.0),
        ),
        icon,
        color,
    );
    if selected {
        paint(
            ui.painter(),
            egui::Rect::from_center_size(
                egui::pos2(response.rect.right() - 22.0, response.rect.center().y),
                egui::vec2(20.0, 20.0),
            ),
            Icon::Check,
            color,
        );
    }
    let tooltip = shortcut.map_or_else(|| label.to_string(), |key| format!("{label} ({key})"));
    response.on_hover_text(tooltip)
}

pub fn menu_action(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    explanation: Option<&str>,
) -> egui::Response {
    let width = ui.available_width().max(220.0);
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.add_sized([width, 44.0], egui::Button::new(format!("    {label}")))
        })
        .inner;
    let accessible_label = explanation.map_or_else(
        || label.to_owned(),
        |reason| format!("{label}. Unavailable: {reason}"),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, accessible_label.clone())
    });
    let color = if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    paint(
        ui.painter(),
        egui::Rect::from_center_size(
            egui::pos2(response.rect.left() + 22.0, response.rect.center().y),
            egui::vec2(20.0, 20.0),
        ),
        icon,
        color,
    );
    match explanation {
        Some(reason) => response.on_hover_text(reason),
        None => response,
    }
}

fn texture(context: &egui::Context, icon: Icon) -> egui::TextureHandle {
    let id = egui::Id::new(("material-symbol-rounded", icon));
    if let Some(texture) = context.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let bytes = icon.png_asset();
    let image = image::load_from_memory(bytes)
        .expect("bundled Material Symbols Rounded PNG is valid")
        .to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    let texture = context.load_texture(
        format!("material-symbol-rounded-{}", icon.asset_name()),
        color_image,
        egui::TextureOptions::LINEAR,
    );
    context.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

impl Icon {
    pub const ALL: [Self; 38] = [
        Self::Brush,
        Self::Smudge,
        Self::ColorMixer,
        Self::Eraser,
        Self::Line,
        Self::Path,
        Self::QuickShape,
        Self::Gradient,
        Self::Rectangle,
        Self::Ellipse,
        Self::RectangleSelection,
        Self::EllipseSelection,
        Self::Lasso,
        Self::Eyedropper,
        Self::Deselect,
        Self::Fill,
        Self::Undo,
        Self::Redo,
        Self::Open,
        Self::Save,
        Self::Layers,
        Self::Color,
        Self::Symmetry,
        Self::Settings,
        Self::Reference,
        Self::DragHandle,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::Eye,
        Self::EyeOff,
        Self::Lock,
        Self::Unlock,
        Self::More,
        Self::EditAnchors,
        Self::AddBox,
        Self::Check,
        Self::Close,
        Self::ExpandMore,
    ];

    pub const fn asset_name(self) -> &'static str {
        match self {
            Self::Brush => "brush",
            Self::Smudge => "touch_app",
            Self::ColorMixer => "blender",
            Self::Eraser => "ink_eraser",
            Self::Line => "diagonal_line",
            Self::Path => "polyline",
            Self::QuickShape => "shapes",
            Self::Gradient => "linear_scale",
            Self::Rectangle => "rectangle",
            Self::Ellipse => "circle",
            Self::RectangleSelection => "select_all",
            Self::EllipseSelection => "radio_button_unchecked",
            Self::Lasso => "lasso_select",
            Self::Eyedropper => "colorize",
            Self::Deselect => "deselect",
            Self::Fill => "format_color_fill",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Open => "folder_open",
            Self::Save => "save",
            Self::Layers => "layers",
            Self::Color => "palette",
            Self::Symmetry => "flip",
            Self::Settings => "settings",
            Self::Reference => "image",
            Self::DragHandle => "drag_indicator",
            Self::ChevronLeft => "chevron_left",
            Self::ChevronRight => "chevron_right",
            Self::Eye => "visibility",
            Self::EyeOff => "visibility_off",
            Self::Lock => "lock",
            Self::Unlock => "lock_open",
            Self::More => "more_horiz",
            Self::EditAnchors => "shape_line",
            Self::AddBox => "add_box",
            Self::Check => "check",
            Self::Close => "close",
            Self::ExpandMore => "expand_more",
        }
    }

    pub const fn svg_asset(self) -> &'static [u8] {
        match self {
            Self::Brush => include_bytes!("../assets/material-symbols-rounded/brush.svg"),
            Self::Smudge => include_bytes!("../assets/material-symbols-rounded/touch_app.svg"),
            Self::ColorMixer => include_bytes!("../assets/material-symbols-rounded/blender.svg"),
            Self::Eraser => include_bytes!("../assets/material-symbols-rounded/ink_eraser.svg"),
            Self::Line => include_bytes!("../assets/material-symbols-rounded/diagonal_line.svg"),
            Self::Path => include_bytes!("../assets/material-symbols-rounded/polyline.svg"),
            Self::QuickShape => include_bytes!("../assets/material-symbols-rounded/shapes.svg"),
            Self::Gradient => include_bytes!("../assets/material-symbols-rounded/linear_scale.svg"),
            Self::Rectangle => include_bytes!("../assets/material-symbols-rounded/rectangle.svg"),
            Self::Ellipse => include_bytes!("../assets/material-symbols-rounded/circle.svg"),
            Self::RectangleSelection => {
                include_bytes!("../assets/material-symbols-rounded/select_all.svg")
            }
            Self::EllipseSelection => {
                include_bytes!("../assets/material-symbols-rounded/radio_button_unchecked.svg")
            }
            Self::Lasso => include_bytes!("../assets/material-symbols-rounded/lasso_select.svg"),
            Self::Eyedropper => include_bytes!("../assets/material-symbols-rounded/colorize.svg"),
            Self::Deselect => include_bytes!("../assets/material-symbols-rounded/deselect.svg"),
            Self::Fill => {
                include_bytes!("../assets/material-symbols-rounded/format_color_fill.svg")
            }
            Self::Undo => include_bytes!("../assets/material-symbols-rounded/undo.svg"),
            Self::Redo => include_bytes!("../assets/material-symbols-rounded/redo.svg"),
            Self::Open => include_bytes!("../assets/material-symbols-rounded/folder_open.svg"),
            Self::Save => include_bytes!("../assets/material-symbols-rounded/save.svg"),
            Self::Layers => include_bytes!("../assets/material-symbols-rounded/layers.svg"),
            Self::Color => include_bytes!("../assets/material-symbols-rounded/palette.svg"),
            Self::Symmetry => include_bytes!("../assets/material-symbols-rounded/flip.svg"),
            Self::Settings => include_bytes!("../assets/material-symbols-rounded/settings.svg"),
            Self::Reference => include_bytes!("../assets/material-symbols-rounded/image.svg"),
            Self::DragHandle => {
                include_bytes!("../assets/material-symbols-rounded/drag_indicator.svg")
            }
            Self::ChevronLeft => {
                include_bytes!("../assets/material-symbols-rounded/chevron_left.svg")
            }
            Self::ChevronRight => {
                include_bytes!("../assets/material-symbols-rounded/chevron_right.svg")
            }
            Self::Eye => include_bytes!("../assets/material-symbols-rounded/visibility.svg"),
            Self::EyeOff => include_bytes!("../assets/material-symbols-rounded/visibility_off.svg"),
            Self::Lock => include_bytes!("../assets/material-symbols-rounded/lock.svg"),
            Self::Unlock => include_bytes!("../assets/material-symbols-rounded/lock_open.svg"),
            Self::More => include_bytes!("../assets/material-symbols-rounded/more_horiz.svg"),
            Self::EditAnchors => {
                include_bytes!("../assets/material-symbols-rounded/shape_line.svg")
            }
            Self::AddBox => include_bytes!("../assets/material-symbols-rounded/add_box.svg"),
            Self::Check => include_bytes!("../assets/material-symbols-rounded/check.svg"),
            Self::Close => include_bytes!("../assets/material-symbols-rounded/close.svg"),
            Self::ExpandMore => {
                include_bytes!("../assets/material-symbols-rounded/expand_more.svg")
            }
        }
    }

    pub const fn png_asset(self) -> &'static [u8] {
        match self {
            Self::Brush => include_bytes!("../assets/material-symbols-rounded/brush.png"),
            Self::Smudge => include_bytes!("../assets/material-symbols-rounded/touch_app.png"),
            Self::ColorMixer => include_bytes!("../assets/material-symbols-rounded/blender.png"),
            Self::Eraser => include_bytes!("../assets/material-symbols-rounded/ink_eraser.png"),
            Self::Line => include_bytes!("../assets/material-symbols-rounded/diagonal_line.png"),
            Self::Path => include_bytes!("../assets/material-symbols-rounded/polyline.png"),
            Self::QuickShape => include_bytes!("../assets/material-symbols-rounded/shapes.png"),
            Self::Gradient => include_bytes!("../assets/material-symbols-rounded/linear_scale.png"),
            Self::Rectangle => include_bytes!("../assets/material-symbols-rounded/rectangle.png"),
            Self::Ellipse => include_bytes!("../assets/material-symbols-rounded/circle.png"),
            Self::RectangleSelection => {
                include_bytes!("../assets/material-symbols-rounded/select_all.png")
            }
            Self::EllipseSelection => {
                include_bytes!("../assets/material-symbols-rounded/radio_button_unchecked.png")
            }
            Self::Lasso => include_bytes!("../assets/material-symbols-rounded/lasso_select.png"),
            Self::Eyedropper => include_bytes!("../assets/material-symbols-rounded/colorize.png"),
            Self::Deselect => include_bytes!("../assets/material-symbols-rounded/deselect.png"),
            Self::Fill => {
                include_bytes!("../assets/material-symbols-rounded/format_color_fill.png")
            }
            Self::Undo => include_bytes!("../assets/material-symbols-rounded/undo.png"),
            Self::Redo => include_bytes!("../assets/material-symbols-rounded/redo.png"),
            Self::Open => include_bytes!("../assets/material-symbols-rounded/folder_open.png"),
            Self::Save => include_bytes!("../assets/material-symbols-rounded/save.png"),
            Self::Layers => include_bytes!("../assets/material-symbols-rounded/layers.png"),
            Self::Color => include_bytes!("../assets/material-symbols-rounded/palette.png"),
            Self::Symmetry => include_bytes!("../assets/material-symbols-rounded/flip.png"),
            Self::Settings => include_bytes!("../assets/material-symbols-rounded/settings.png"),
            Self::Reference => include_bytes!("../assets/material-symbols-rounded/image.png"),
            Self::DragHandle => {
                include_bytes!("../assets/material-symbols-rounded/drag_indicator.png")
            }
            Self::ChevronLeft => {
                include_bytes!("../assets/material-symbols-rounded/chevron_left.png")
            }
            Self::ChevronRight => {
                include_bytes!("../assets/material-symbols-rounded/chevron_right.png")
            }
            Self::Eye => include_bytes!("../assets/material-symbols-rounded/visibility.png"),
            Self::EyeOff => include_bytes!("../assets/material-symbols-rounded/visibility_off.png"),
            Self::Lock => include_bytes!("../assets/material-symbols-rounded/lock.png"),
            Self::Unlock => include_bytes!("../assets/material-symbols-rounded/lock_open.png"),
            Self::More => include_bytes!("../assets/material-symbols-rounded/more_horiz.png"),
            Self::EditAnchors => {
                include_bytes!("../assets/material-symbols-rounded/shape_line.png")
            }
            Self::AddBox => include_bytes!("../assets/material-symbols-rounded/add_box.png"),
            Self::Check => include_bytes!("../assets/material-symbols-rounded/check.png"),
            Self::Close => include_bytes!("../assets/material-symbols-rounded/close.png"),
            Self::ExpandMore => {
                include_bytes!("../assets/material-symbols-rounded/expand_more.png")
            }
        }
    }
}

/// Draw an icon-only button while retaining its human-readable button label for
/// hover, focus, and accessibility information.
pub fn button(
    ui: &mut egui::Ui,
    icon: Icon,
    name: &str,
    shortcut: Option<&str>,
    selected: bool,
    size: [f32; 2],
) -> egui::Response {
    let hidden_label = egui::RichText::new(name)
        .size(1.0)
        .color(egui::Color32::TRANSPARENT);
    let size = [size[0].max(44.0), size[1].max(44.0)];
    let response = ui.add_sized(size, egui::Button::new(hidden_label).selected(selected));
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, name)
    });
    let color = if selected {
        ui.visuals().selection.stroke.color
    } else if response.hovered() {
        ui.visuals().widgets.hovered.fg_stroke.color
    } else {
        ui.visuals().text_color()
    };
    paint(ui.painter(), response.rect.shrink(9.0), icon, color);
    let tooltip = shortcut.map_or_else(|| name.to_string(), |key| format!("{name} ({key})"));
    response.on_hover_text(tooltip)
}

/// Draw a recognizable, font-independent palette grip while leaving dragging
/// to the containing movable Area. The semantic label gives screen readers a
/// named point to locate the moveable tool palette.
pub fn palette_drag_handle(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::drag());
    paint(
        ui.painter(),
        rect.shrink(10.0),
        Icon::DragHandle,
        ui.visuals().text_color(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Move tool palette")
    });
    response.on_hover_text("Drag this palette to move it; release near an edge to dock")
}

/// Draw a pointer drag grip without exposing a keyboard no-op as a button.
/// Layer reorder also remains available through the row's keyboard-accessible
/// Actions menu.
pub fn layer_drag_handle(ui: &mut egui::Ui, name: &str, size: [f32; 2]) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(size[0].max(44.0), size[1].max(44.0)),
        egui::Sense::hover(),
    );
    paint(
        ui.painter(),
        rect.shrink(9.0),
        Icon::DragHandle,
        ui.visuals().text_color(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Label,
            true,
            format!("Drag {name} with a pointer to reorder; use Actions for keyboard reordering"),
        )
    });
    response.on_hover_text(format!(
        "Drag {name} to reorder. Keyboard: use the Actions menu."
    ))
}

/// Add a vector mark to a normal text button. The text remains the accessible
/// button label and the complete row remains clickable.
pub fn menu_button(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    menu_button_enabled(ui, icon, label, true)
}

pub fn menu_button_enabled(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui.add_enabled(enabled, egui::Button::new(format!("    {label}")));
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(response.rect.left() + 14.0, response.rect.center().y),
        egui::vec2(17.0, 17.0),
    );
    let color = if response.hovered() {
        ui.visuals().widgets.hovered.fg_stroke.color
    } else {
        ui.visuals().text_color()
    };
    paint(ui.painter(), icon_rect, icon, color);
    response
}

pub fn color_swatch(ui: &mut egui::Ui, color: egui::Color32, label: &str) -> egui::Response {
    let semantic_label = egui::RichText::new(label)
        .size(1.0)
        .color(egui::Color32::TRANSPARENT);
    let response = ui.add_sized([44.0, 44.0], egui::Button::new(semantic_label).fill(color));
    let border = if response.hovered() || response.has_focus() {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    ui.painter().rect_stroke(
        response.rect.shrink(1.0),
        5.0,
        border,
        egui::StrokeKind::Inside,
    );
    response.on_hover_text(label)
}

pub fn label(ui: &mut egui::Ui, icon: Icon, name: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
    paint(
        ui.painter(),
        rect.shrink(2.0),
        icon,
        ui.visuals().text_color(),
    );
    ui.label(name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_approved_material_symbol_asset_decodes_and_renders() {
        let context = egui::Context::default();
        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                for icon in Icon::ALL {
                    button(ui, icon, "Tool name", Some("Shortcut"), false, [36.0, 36.0]);
                }
            });
        });
        assert!(output.shapes.len() >= Icon::ALL.len());
        for icon in Icon::ALL {
            assert!(
                icon.svg_asset().starts_with(b"<svg"),
                "{} SVG",
                icon.asset_name()
            );
            let image = image::load_from_memory(icon.png_asset())
                .unwrap_or_else(|error| panic!("{} PNG: {error}", icon.asset_name()));
            assert_eq!(
                (image.width(), image.height()),
                (96, 96),
                "{} texture size",
                icon.asset_name()
            );
            assert!(
                image.to_rgba8().pixels().any(|pixel| pixel[3] != 0),
                "{} is blank",
                icon.asset_name()
            );
        }
        assert_eq!(Icon::Smudge.asset_name(), "touch_app");
    }

    #[test]
    fn grouped_tool_controls_emit_named_accessible_toolbar_and_radio_options() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut bounds = Vec::new();
        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                let toolbar = egui::Id::new("test-toolbar");
                context.accesskit_node_builder(toolbar, |node| {
                    node.set_role(egui::accesskit::Role::Toolbar);
                    node.set_label("Tools");
                });
                ui.scope_builder(egui::UiBuilder::new().accessibility_parent(toolbar), |ui| {
                    let group = egui::Id::new("test-paint-effects");
                    let trigger =
                        group_button(ui, Icon::Smudge, "Paint Effects", "Smudge", 2, true);
                    bounds.push(trigger.rect);
                    context.accesskit_node_builder(group, |node| {
                        node.set_role(egui::accesskit::Role::RadioGroup);
                        node.set_label("Paint Effects");
                    });
                    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(group), |ui| {
                        let smudge = menu_choice(ui, Icon::Smudge, "Smudge", Some("S"), true);
                        bounds.push(smudge.rect);
                        let mixer =
                            menu_choice(ui, Icon::ColorMixer, "Color mixer", Some("X"), false);
                        bounds.push(mixer.rect);
                    });
                });
            });
        });
        let update = output
            .platform_output
            .accesskit_update
            .expect("AccessKit-enabled UI must produce a tree update");
        let nodes: Vec<_> = update.nodes.iter().map(|(_, node)| node).collect();
        let labelled = |name: &str| {
            nodes
                .iter()
                .find(|node| node.label() == Some(name))
                .unwrap_or_else(|| panic!("missing accessible node label {name:?}"))
        };
        assert_eq!(labelled("Tools").role(), egui::accesskit::Role::Toolbar);
        assert_eq!(
            labelled("Paint Effects").role(),
            egui::accesskit::Role::RadioGroup
        );
        assert_eq!(
            labelled("Smudge").role(),
            egui::accesskit::Role::RadioButton
        );
        assert_eq!(
            labelled("Color mixer").role(),
            egui::accesskit::Role::RadioButton
        );
        assert!(
            bounds
                .iter()
                .all(|rect| rect.width() >= 44.0 && rect.height() >= 44.0)
        );
    }

    #[test]
    fn recent_color_swatch_has_a_touch_sized_named_button() {
        let context = egui::Context::default();
        let mut width = 0.0;
        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                let response = color_swatch(
                    ui,
                    egui::Color32::from_rgb(160, 80, 210),
                    "Brush color #A050D2",
                );
                width = response.rect.width();
                assert!(response.enabled());
            });
        });
        assert!(width >= 44.0);
        assert!(!output.shapes.is_empty());
    }

    #[test]
    fn palette_drag_handle_is_a_touch_sized_named_control() {
        let context = egui::Context::default();
        let mut bounds = egui::Rect::NOTHING;
        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                bounds = palette_drag_handle(ui).rect;
            });
        });
        assert!(bounds.width() >= 44.0 && bounds.height() >= 44.0);
        assert!(
            !output
                .shapes
                .iter()
                .any(|shape| { matches!(shape.shape, egui::epaint::Shape::Text(_)) })
        );
    }
}
