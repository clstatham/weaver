#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum GuiElement {
    Button {
        text: String,
        // on_click: Box<dyn FnMut()>,
    },
    Text {
        text: String,
    },
    Slider {
        value: f32,
        min: f32,
        max: f32,
        // on_change: Box<dyn FnMut(f32)>,
    },
    Checkbox {
        value: bool,
        // on_change: Box<dyn FnMut(bool)>,
    },
    Window {
        title: String,
        elements: Vec<GuiElement>,
    },
}

impl GuiElement {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        match self {
            Self::Button { text } => {
                if ui.button(text.to_owned()).clicked() {
                    // on_click();
                }
            }
            Self::Text { text } => {
                ui.label(text.to_owned());
            }
            Self::Slider {
                value: original_value,
                min,
                max,
            } => {
                let mut value = *original_value;
                ui.add(
                    egui::Slider::new(&mut value, *min..=*max).text(format!("{}: {}", min, max)),
                );
                if value != *original_value {
                    *original_value = value;
                }
            }
            Self::Checkbox {
                value: original_value,
            } => {
                let mut value = *original_value;
                ui.checkbox(&mut value, "");
                if value != *original_value {
                    *original_value = value;
                }
            }
            Self::Window { title, elements } => {
                egui::Window::new(title.to_owned()).show(ui.ctx(), |ui| {
                    for element in elements {
                        element.ui(ui);
                    }
                });
            }
        }
    }
}
