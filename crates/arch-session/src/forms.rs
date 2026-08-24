use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ControlId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FormMethod {
    Get,
    Post,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ControlKind {
    Text,
    Password,
    Checkbox,
    Radio,
    Select,
    Submit,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormControl {
    pub id: ControlId,
    pub name: Option<String>,
    pub kind: ControlKind,
    pub value: String,
    pub checked: bool,
    pub options: Vec<SelectOption>,
    pub selected_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormSubmission {
    pub method: FormMethod,
    pub target: Url,
    pub encoded: String,
}

#[derive(Debug, Error)]
pub enum FormError {
    #[error("form control does not exist")]
    MissingControl,
    #[error("form control does not accept text")]
    NotTextControl,
    #[error("form control is not selectable")]
    NotSelectable,
    #[error("select option does not exist")]
    MissingOption,
    #[error("submitter is not a submit control")]
    InvalidSubmitter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormState {
    pub action: Url,
    pub method: FormMethod,
    pub controls: Vec<FormControl>,
    dirty: bool,
}

impl FormState {
    #[must_use]
    pub const fn new(action: Url, method: FormMethod, controls: Vec<FormControl>) -> Self {
        Self {
            action,
            method,
            controls,
            dirty: false,
        }
    }

    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Changes a text or password control.
    ///
    /// # Errors
    /// Returns [`FormError`] when the control does not exist or does not accept text.
    pub fn set_text(&mut self, id: ControlId, value: String) -> Result<(), FormError> {
        let control = self.control_mut(id)?;
        if !matches!(control.kind, ControlKind::Text | ControlKind::Password) {
            return Err(FormError::NotTextControl);
        }
        control.value = value;
        self.dirty = true;
        Ok(())
    }

    /// Changes a checkbox or radio control.
    ///
    /// # Errors
    /// Returns [`FormError`] when the control does not exist or is not checkable.
    pub fn set_checked(&mut self, id: ControlId, checked: bool) -> Result<(), FormError> {
        let (kind, name) = {
            let control = self.control_mut(id)?;
            if !matches!(control.kind, ControlKind::Checkbox | ControlKind::Radio) {
                return Err(FormError::NotSelectable);
            }
            control.checked = checked;
            (control.kind, control.name.clone())
        };
        if checked
            && kind == ControlKind::Radio
            && let Some(name) = name
        {
            for control in &mut self.controls {
                if control.id != id
                    && control.kind == ControlKind::Radio
                    && control.name.as_deref() == Some(&name)
                {
                    control.checked = false;
                }
            }
        }
        self.dirty = true;
        Ok(())
    }

    /// Selects one option by index.
    ///
    /// # Errors
    /// Returns [`FormError`] when the control or option does not exist or is not a select control.
    pub fn select(&mut self, id: ControlId, index: usize) -> Result<(), FormError> {
        let control = self.control_mut(id)?;
        if control.kind != ControlKind::Select {
            return Err(FormError::NotSelectable);
        }
        if index >= control.options.len() {
            return Err(FormError::MissingOption);
        }
        control.selected_index = Some(index);
        self.dirty = true;
        Ok(())
    }

    /// Builds a user-initiated GET or form-urlencoded POST submission.
    ///
    /// # Errors
    /// Returns [`FormError`] when the selected submitter is missing or not a submit control.
    pub fn submission(&self, submitter: Option<ControlId>) -> Result<FormSubmission, FormError> {
        if let Some(submitter) = submitter
            && !self
                .controls
                .iter()
                .any(|control| control.id == submitter && control.kind == ControlKind::Submit)
        {
            return Err(FormError::InvalidSubmitter);
        }
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for control in &self.controls {
            let Some(name) = control.name.as_deref() else {
                continue;
            };
            let value = match control.kind {
                ControlKind::Text | ControlKind::Password => Some(control.value.as_str()),
                ControlKind::Checkbox | ControlKind::Radio if control.checked => {
                    Some(control.value.as_str())
                }
                ControlKind::Select => control
                    .selected_index
                    .and_then(|index| control.options.get(index))
                    .map(|option| option.value.as_str()),
                ControlKind::Submit if Some(control.id) == submitter => {
                    Some(control.value.as_str())
                }
                ControlKind::Checkbox | ControlKind::Radio | ControlKind::Submit => None,
            };
            if let Some(value) = value {
                serializer.append_pair(name, value);
            }
        }
        let encoded = serializer.finish();
        let mut target = self.action.clone();
        if self.method == FormMethod::Get {
            target.set_query((!encoded.is_empty()).then_some(encoded.as_str()));
        }
        Ok(FormSubmission {
            method: self.method,
            target,
            encoded,
        })
    }

    fn control_mut(&mut self, id: ControlId) -> Result<&mut FormControl, FormError> {
        self.controls
            .iter_mut()
            .find(|control| control.id == id)
            .ok_or(FormError::MissingControl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(method: FormMethod) -> FormState {
        FormState {
            action: Url::parse("https://example.com/search?old=ignored").unwrap(),
            method,
            controls: vec![
                FormControl {
                    id: ControlId(1),
                    name: Some("query".to_owned()),
                    kind: ControlKind::Text,
                    value: "rust browser".to_owned(),
                    checked: false,
                    options: Vec::new(),
                    selected_index: None,
                },
                FormControl {
                    id: ControlId(2),
                    name: Some("remember".to_owned()),
                    kind: ControlKind::Checkbox,
                    value: "yes".to_owned(),
                    checked: true,
                    options: Vec::new(),
                    selected_index: None,
                },
                FormControl {
                    id: ControlId(3),
                    name: Some("ignored".to_owned()),
                    kind: ControlKind::Checkbox,
                    value: "no".to_owned(),
                    checked: false,
                    options: Vec::new(),
                    selected_index: None,
                },
                FormControl {
                    id: ControlId(4),
                    name: Some("go".to_owned()),
                    kind: ControlKind::Submit,
                    value: "Search".to_owned(),
                    checked: false,
                    options: Vec::new(),
                    selected_index: None,
                },
            ],
            dirty: false,
        }
    }

    #[test]
    fn serializes_only_successful_controls_for_get() {
        let submission = form(FormMethod::Get)
            .submission(Some(ControlId(4)))
            .unwrap();
        assert_eq!(
            submission.target.as_str(),
            "https://example.com/search?query=rust+browser&remember=yes&go=Search"
        );
        assert_eq!(
            submission.encoded,
            "query=rust+browser&remember=yes&go=Search"
        );
    }

    #[test]
    fn keeps_post_body_out_of_the_target_url() {
        let submission = form(FormMethod::Post).submission(None).unwrap();
        assert_eq!(
            submission.target.as_str(),
            "https://example.com/search?old=ignored"
        );
        assert_eq!(submission.encoded, "query=rust+browser&remember=yes");
    }

    #[test]
    fn updates_text_radio_and_select_state() {
        let mut state = form(FormMethod::Post);
        state.controls.extend([
            FormControl {
                id: ControlId(5),
                name: Some("mode".to_owned()),
                kind: ControlKind::Radio,
                value: "one".to_owned(),
                checked: true,
                options: Vec::new(),
                selected_index: None,
            },
            FormControl {
                id: ControlId(6),
                name: Some("mode".to_owned()),
                kind: ControlKind::Radio,
                value: "two".to_owned(),
                checked: false,
                options: Vec::new(),
                selected_index: None,
            },
            FormControl {
                id: ControlId(7),
                name: Some("size".to_owned()),
                kind: ControlKind::Select,
                value: String::new(),
                checked: false,
                options: vec![
                    SelectOption {
                        value: "s".to_owned(),
                        label: "Small".to_owned(),
                    },
                    SelectOption {
                        value: "l".to_owned(),
                        label: "Large".to_owned(),
                    },
                ],
                selected_index: Some(0),
            },
        ]);
        state.set_text(ControlId(1), "updated".to_owned()).unwrap();
        state.set_checked(ControlId(6), true).unwrap();
        state.select(ControlId(7), 1).unwrap();
        let submission = state.submission(None).unwrap();
        assert!(state.is_dirty());
        assert!(submission.encoded.contains("query=updated"));
        assert!(submission.encoded.contains("mode=two"));
        assert!(!submission.encoded.contains("mode=one"));
        assert!(submission.encoded.contains("size=l"));
    }
}
