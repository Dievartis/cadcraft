//! Guided access to dynamic-block authoring commands.

use crate::CadApp;
use crate::theme::Tokens;
use egui::{RichText, vec2};

/// The BParameter helper keeps the command prompts discoverable while retaining
/// the command-line workflow and its automation interface.
pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    let t = Tokens::get();
    let mut launch = None;
    egui::Window::new(crate::tl!("Block parameters")).open(open).resizable(false).default_width(360.0).show(ctx, |ui| {
        ui.label(RichText::new(crate::tl!("Choose a parameter to add or edit in the block editor.")).color(t.text_dim));
        ui.add_space(8.0);
        for (icon, label, help, command) in [
            (
                "↔",
                crate::tl!("Linear repetition"),
                crate::tl!("Select the direction, enter the spacing and choose the number of occurrences."),
                "bparameter",
            ),
            ("•", crate::tl!("Position parameter"), crate::tl!("Place a point parameter to move an element in the block."), "bparameter"),
            ("↕", crate::tl!("Visibility parameter"), crate::tl!("Create visibility states for alternative block contents."), "bparameter"),
            ("≋", crate::tl!("Lookup table"), crate::tl!("Define a table that maps a property value to block dimensions."), "blookup"),
        ] {
            ui.horizontal(|ui| {
                if ui.button(RichText::new(icon).size(18.0)).on_hover_text(help).clicked() {
                    launch = Some(command);
                }
                ui.label(label).on_hover_text(help);
            });
        }
        ui.add_space(8.0);
        ui.label(crate::tl!("Open a block first with BEDIT. The prompts then guide the selection and spacing."));
        ui.set_min_size(vec2(340.0, 0.0));
    });
    if let Some(command) = launch {
        *open = false;
        app.ui.dialog = None;
        app.start(command);
    }
}
