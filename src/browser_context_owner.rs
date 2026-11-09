//! Per-cohort native context routing. Only an actual captured frame supplies the
//! GUI's expected binding; observed protocol packets can never mount a GUI item.
use crate::{browser_wayland_context as native, gui_input_context as gui, json, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
pub(crate) type Shared = Arc<Mutex<Owner>>;
pub(crate) struct FrameInput<'a> {
    pub thread: &'a str,
    pub tab: &'a str,
    pub scope: &'a str,
    pub revision: u64,
    pub title: &'a str,
    pub content: &'a serde_json::Value,
}
/// Explicit transport boundary: production uses only the admitted GUI lease.
/// Pure tests use a recording sink and never impersonate kernel admission.
pub(crate) trait Sink {
    fn publish(&mut self, binding: &gui::Binding, state: &gui::State<'_>) -> Result<()>;
    fn invalidate(&mut self, binding: &gui::Binding) -> Result<()>;
    fn request(&mut self) -> Result<Option<gui::Binding>>;
}
impl Sink for gui::Lease {
    fn publish(&mut self, binding: &gui::Binding, state: &gui::State<'_>) -> Result<()> {
        gui::Lease::publish(self, binding, state)
    }
    fn invalidate(&mut self, binding: &gui::Binding) -> Result<()> {
        gui::Lease::invalidate(self, binding)
    }
    fn request(&mut self) -> Result<Option<gui::Binding>> {
        gui::Lease::request(self)
    }
}
struct Frame {
    binding: gui::Binding,
    source: (String, u32),
}
pub(crate) struct Owner<S = gui::Lease> {
    id: String,
    lease: S,
    tabs: HashMap<(String, String), String>,
    titles: HashMap<String, String>,
    states: HashMap<(String, u32), native::Snapshot>,
    frame: Option<Frame>,
}
fn error() -> String {
    "browser_input_context_unavailable".into()
}
impl Owner {
    pub(crate) fn register(
        gui: crate::gui_clipboard::Shared,
        id: &str,
        engine: &crate::browser_engine_identity::Engine,
    ) -> Result<Shared> {
        Ok(Arc::new(Mutex::new(Self::new(
            id,
            gui::Lease::register(gui, id, engine)?,
        ))))
    }
}
impl<S: Sink> Owner<S> {
    fn new(id: &str, lease: S) -> Self {
        Self {
            id: id.into(),
            lease,
            tabs: HashMap::new(),
            titles: HashMap::new(),
            states: HashMap::new(),
            frame: None,
        }
    }
    pub(crate) fn titles(&self) -> HashMap<String, String> {
        self.titles.clone()
    }
    pub(crate) fn title(&mut self, thread: &str, tab: &str) -> Result<String> {
        let key = (thread.to_owned(), tab.to_owned());
        if let Some(token) = self.tabs.get(&key) {
            return Ok(format!("O3-native-{token}"));
        }
        if self.tabs.len() >= 32 {
            return Err(error());
        }
        let token = uuid::Uuid::new_v4().to_string();
        let title = format!("O3-native-{token}");
        self.tabs.insert(key, token.clone());
        self.titles.insert(title.clone(), token);
        Ok(title)
    }
    pub(crate) fn hide(&mut self) -> Result<()> {
        if let Some(frame) = self.frame.take() {
            self.lease.invalidate(&frame.binding)?;
        }
        Ok(())
    }
    pub(crate) fn remove(&mut self, thread: &str, tab: &str) -> Result<()> {
        if let Some(token) = self.tabs.remove(&(thread.into(), tab.into())) {
            if self
                .frame
                .as_ref()
                .is_some_and(|frame| frame.binding.tab == token)
            {
                self.hide()?;
            }
            self.titles.remove(&format!("O3-native-{token}"));
            self.states.retain(|_, state| state.lease != token);
        }
        Ok(())
    }
    pub(crate) fn update(
        &mut self,
        connection: &str,
        invalidated: std::collections::HashSet<u32>,
        states: HashMap<u32, native::Snapshot>,
    ) -> Result<()> {
        for object in invalidated {
            let key = (connection.to_owned(), object);
            if self.frame.as_ref().is_some_and(|frame| frame.source == key) {
                self.hide()?;
            }
            self.states.remove(&key);
        }
        let mut refresh = false;
        for (object, state) in states {
            refresh |= self
                .frame
                .as_ref()
                .is_some_and(|frame| frame.source == (connection.to_owned(), object));
            if state.object != object {
                return Err(error());
            }
            self.states.insert((connection.into(), object), state);
        }
        if refresh {
            self.publish_current()?;
        }
        self.requests()
    }
    pub(crate) fn close_connection(&mut self, connection: &str) -> Result<()> {
        if self
            .frame
            .as_ref()
            .is_some_and(|frame| frame.source.0 == connection)
        {
            self.hide()?;
        }
        self.states.retain(|(source, _), _| source != connection);
        Ok(())
    }
    /// `content` is reported only by the trusted host container/native window,
    /// never by target DOM/AX. This function is called only after frame admission.
    pub(crate) fn frame(
        &mut self,
        input: FrameInput<'_>,
    ) -> Result<Option<(serde_json::Value, serde_json::Value)>> {
        let FrameInput {
            thread,
            tab,
            scope,
            revision,
            title,
            content,
        } = input;
        self.hide()?;
        let Some(token) = self.tabs.get(&(thread.into(), tab.into())) else {
            return Ok(None);
        };
        if self.titles.get(title) != Some(token) {
            return Ok(None);
        }
        let Some(row) = content.as_object().filter(|row| row.len() == 4) else {
            return Ok(None);
        };
        let mut rect = [0.0; 4];
        for (index, name) in ["x", "y", "width", "height"].iter().enumerate() {
            let Some(value) = row
                .get(*name)
                .and_then(serde_json::Value::as_f64)
                .filter(|value| value.is_finite() && value.abs() <= 32768.0)
            else {
                return Ok(None);
            };
            rect[index] = value;
        }
        if rect[2] <= 0.0 || rect[3] <= 0.0 {
            return Ok(None);
        }
        let mut matching = self
            .states
            .iter()
            .filter(|(_, state)| &state.lease == token);
        let Some((source, state)) = matching.next() else {
            return Ok(None);
        };
        // Multiple active contexts or a child surface require actual transform
        // evidence; refuse query metadata instead of inventing an offset.
        if matching.next().is_some() || state.surface != state.root {
            return Ok(None);
        }
        let binding = gui::Binding {
            scope: scope.into(),
            connection: source.0.clone(),
            surface: format!("{}:{}", state.root, state.surface),
            tab: token.clone(),
            frame: revision.to_string(),
        };
        let metadata = json!({"connection":binding.connection,"surface":binding.surface,"tab":binding.tab,"frame":binding.frame});
        self.frame = Some(Frame {
            binding,
            source: source.clone(),
        });
        self.requests()?;
        Ok(Some((metadata, content.clone())))
    }
    pub(crate) fn connection(&self) -> String {
        format!("{}:{}", self.id, uuid::Uuid::new_v4())
    }
    fn publish_current(&mut self) -> Result<()> {
        let Some(frame) = &self.frame else {
            return Ok(());
        };
        let Some(source) = self.states.get(&frame.source) else {
            return Ok(());
        };
        let surrounding = source
            .state
            .surrounding
            .as_ref()
            .map(|(text, cursor, anchor)| gui::Surrounding {
                text,
                cursor_byte: *cursor,
                anchor_byte: *anchor,
            });
        let (hints, purpose) = source.state.content.unwrap_or((0, 0));
        self.lease.publish(
            &frame.binding,
            &gui::State {
                serial: source.serial,
                caret: source.state.caret,
                surrounding,
                hints: hints as u16,
                purpose: purpose as u8,
            },
        )
    }
    /// No hidden queue or replay. GUI requests must match the current complete
    /// frame binding; unsolicited protocol updates do not select another frame.
    pub(crate) fn requests(&mut self) -> Result<()> {
        for _ in 0..16 {
            let Some(request) = self.lease.request()? else {
                return Ok(());
            };
            if !self
                .frame
                .as_ref()
                .is_some_and(|frame| frame.binding == request)
            {
                continue;
            }
            self.publish_current()?;
        }
        Err(error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};
    #[derive(Default)]
    struct RecordingSink {
        requests: VecDeque<gui::Binding>,
        published: Vec<gui::Binding>,
        invalidated: Vec<gui::Binding>,
    }
    impl Sink for RecordingSink {
        fn publish(&mut self, binding: &gui::Binding, _: &gui::State<'_>) -> Result<()> {
            self.published.push(binding.clone());
            Ok(())
        }
        fn invalidate(&mut self, binding: &gui::Binding) -> Result<()> {
            self.invalidated.push(binding.clone());
            Ok(())
        }
        fn request(&mut self) -> Result<Option<gui::Binding>> {
            Ok(self.requests.pop_front())
        }
    }
    fn state(token: String) -> native::Snapshot {
        native::Snapshot {
            object: 4,
            surface: 2,
            root: 2,
            lease: token,
            serial: 1,
            state: native::State {
                caret: Some([23, 63, 0, 18]),
                surrounding: Some(("A😀日本ZQ".into(), 13, 13)),
                content: Some((4, 0)),
            },
        }
    }
    fn fixture() -> (Owner<RecordingSink>, String, String) {
        let mut owner = Owner::new("cohort", RecordingSink::default());
        let title = owner.title("thread", "tab").unwrap();
        let token = owner.titles[&title].clone();
        (owner, title, token)
    }
    #[test]
    fn packets_cannot_mount_without_actual_frame_and_complete_binding() {
        let (mut owner, title, token) = fixture();
        let connection = owner.connection();
        owner
            .update(
                &connection,
                HashSet::new(),
                HashMap::from([(4, state(token))]),
            )
            .unwrap();
        assert!(owner.frame.is_none());
        assert!(owner.lease.published.is_empty());
        let content = json!({"x":0,"y":29,"width":1280,"height":995});
        let (metadata, _) = owner
            .frame(FrameInput {
                thread: "thread",
                tab: "tab",
                scope: "real-request",
                revision: 1,
                title: &title,
                content: &content,
            })
            .unwrap()
            .unwrap();
        assert_eq!(metadata.as_object().unwrap().len(), 4);
        assert!(metadata.get("scope").is_none());
        assert!(owner.lease.published.is_empty());
        let expected = owner.frame.as_ref().unwrap().binding.clone();
        let mut foreign = expected.clone();
        foreign.frame = "old-frame".into();
        owner.lease.requests.extend([foreign, expected.clone()]);
        owner.requests().unwrap();
        assert!(owner.lease.published == vec![expected]);
    }
    #[test]
    fn hide_destruction_and_replacement_invalidate_before_new_binding() {
        let (mut owner, title, token) = fixture();
        let connection = owner.connection();
        owner
            .update(
                &connection,
                HashSet::new(),
                HashMap::from([(4, state(token))]),
            )
            .unwrap();
        let content = json!({"x":0,"y":29,"width":1280,"height":995});
        owner
            .frame(FrameInput {
                thread: "thread",
                tab: "tab",
                scope: "request",
                revision: 1,
                title: &title,
                content: &content,
            })
            .unwrap();
        let old = owner.frame.as_ref().unwrap().binding.clone();
        owner.hide().unwrap();
        assert!(owner.frame.is_none());
        assert!(owner.lease.invalidated == vec![old.clone()]);
        owner.lease.requests.push_back(old);
        owner.requests().unwrap();
        assert!(owner.lease.published.is_empty());
        owner
            .frame(FrameInput {
                thread: "thread",
                tab: "tab",
                scope: "request",
                revision: 2,
                title: &title,
                content: &content,
            })
            .unwrap();
        owner
            .update(&connection, HashSet::from([4]), HashMap::new())
            .unwrap();
        assert!(owner.frame.is_none());
        owner.remove("thread", "tab").unwrap();
        assert!(owner.titles.is_empty());
        assert!(owner.states.is_empty());
    }
    #[test]
    fn missing_or_unproved_geometry_and_ambiguous_surfaces_remain_unknown() {
        let (mut owner, title, token) = fixture();
        let connection = owner.connection();
        owner
            .update(
                &connection,
                HashSet::new(),
                HashMap::from([(4, state(token.clone()))]),
            )
            .unwrap();
        let content = json!({"x":0,"y":29,"width":1280,"height":995});
        assert!(owner
            .frame(FrameInput {
                thread: "thread",
                tab: "tab",
                scope: "request",
                revision: 1,
                title: "foreign",
                content: &content
            })
            .unwrap()
            .is_none());
        assert!(owner
            .frame(FrameInput {
                thread: "thread",
                tab: "tab",
                scope: "request",
                revision: 1,
                title: &title,
                content: &json!({"x":0,"y":0,"width":0,"height":995})
            })
            .unwrap()
            .is_none());
        let mut child = state(token.clone());
        child.surface = 3;
        owner.states.insert((connection.clone(), 4), child);
        assert!(owner
            .frame(FrameInput {
                thread: "thread",
                tab: "tab",
                scope: "request",
                revision: 1,
                title: &title,
                content: &content
            })
            .unwrap()
            .is_none());
        owner
            .states
            .insert((connection.clone(), 4), state(token.clone()));
        let mut second = state(token);
        second.object = 5;
        owner.states.insert((connection, 5), second);
        assert!(owner
            .frame(FrameInput {
                thread: "thread",
                tab: "tab",
                scope: "request",
                revision: 1,
                title: &title,
                content: &content
            })
            .unwrap()
            .is_none());
    }
}
