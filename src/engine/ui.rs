//! Engine-facing bridge for the retained UI runtime.

use crate::engine::Engine;
use crate::types::Input;
use crate::ui::{UiPointerPhase, UiViewport};

impl Engine {
    pub(crate) fn set_input(&mut self, input: Input) {
        self.input = input;
    }

    pub(crate) fn set_ui_viewport(&mut self, viewport: UiViewport) {
        self.ui.borrow_mut().set_viewport(viewport);
    }

    pub(crate) fn set_ui_suppressed(&mut self, suppressed: bool) {
        self.ui.borrow_mut().set_suppressed(suppressed);
    }

    pub(crate) fn set_authenticated(&mut self, authenticated: bool) {
        self.ui.borrow_mut().set_authenticated(authenticated);
    }

    pub(crate) fn ui_node_count(&self) -> usize {
        self.ui.borrow().document_node_count()
    }

    pub(crate) fn ui_hit_test(&mut self, x: f32, y: f32) -> bool {
        self.ui.borrow_mut().is_interactive_at(x, y)
    }

    pub(crate) fn ui_external_link_hit_test(&mut self, x: f32, y: f32) -> bool {
        self.ui.borrow_mut().is_external_link_at(x, y)
    }

    pub(crate) fn ui_shared_modal_visible(&self) -> bool {
        self.ui.borrow().shared_modal_visible()
    }

    pub(crate) fn set_ui_document(&mut self, source: &str) -> bool {
        self.ui.borrow_mut().set_document_json(source).is_ok()
    }

    pub(crate) fn prepare_ui_document_buffer(&mut self, length: usize) -> *mut u8 {
        self.ui_document_buffer.resize(length, 0);
        self.ui_document_buffer.as_mut_ptr()
    }

    pub(crate) fn load_ui_document_buffer(&mut self) -> bool {
        let source = String::from_utf8_lossy(&self.ui_document_buffer).into_owned();
        self.set_ui_document(&source)
    }

    pub(crate) fn ui_pointer(
        &mut self,
        pointer_id: u64,
        phase: UiPointerPhase,
        x: f32,
        y: f32,
    ) -> bool {
        self.ui.borrow_mut().pointer(pointer_id, phase, x, y)
    }
}
