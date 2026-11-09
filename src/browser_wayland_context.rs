//! Current text-input-v3 metadata observed on an authenticated engine stream.
//! Forwarding remains byte-for-byte opaque; no text is logged or exposed to QML.
use std::collections::{HashMap, HashSet};
type Result<T> = std::result::Result<T, &'static str>;
const OBJECT_LIMIT: usize = 8192;
const MESSAGE_LIMIT: usize = 65532;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Interface {
    Display,
    Registry,
    Compositor,
    Subcompositor,
    Surface,
    Subsurface,
    Shell,
    XdgSurface,
    Toplevel,
    TextManager,
    Text,
}
#[derive(Clone, Default)]
pub(crate) struct State {
    pub caret: Option<[i32; 4]>,
    pub surrounding: Option<(String, u32, u32)>,
    pub content: Option<(u32, u32)>,
}
impl State {
    fn merge(&mut self, pending: Self) {
        if pending.caret.is_some() {
            self.caret = pending.caret;
        }
        if pending.surrounding.is_some() {
            self.surrounding = pending.surrounding;
        }
        if pending.content.is_some() {
            self.content = pending.content;
        }
    }
}
#[derive(Default)]
struct Context {
    surface: Option<u32>,
    enabled: bool,
    count: u32,
    current: State,
    pending: State,
    sent: Option<(u32, Option<u32>, State)>,
}
pub(crate) struct Snapshot {
    pub object: u32,
    pub surface: u32,
    pub root: u32,
    pub lease: String,
    pub serial: u32,
    pub state: State,
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn word(&mut self) -> Result<u32> {
        let bytes = self
            .bytes
            .get(self.at..self.at + 4)
            .ok_or("native_message_truncated")?;
        self.at += 4;
        Ok(u32::from_ne_bytes(
            bytes.try_into().map_err(|_| "native_message_truncated")?,
        ))
    }
    fn string(&mut self) -> Result<String> {
        let length = self.word()? as usize;
        if length == 0 {
            return Err("native_string_shape");
        }
        let padded = length.checked_add(3).ok_or("native_string_shape")? & !3;
        let bytes = self
            .bytes
            .get(self.at..self.at.checked_add(padded).ok_or("native_string_shape")?)
            .ok_or("native_string_shape")?;
        self.at += padded;
        if bytes[length - 1] != 0 || bytes[..length - 1].contains(&0) {
            return Err("native_string_shape");
        }
        String::from_utf8(bytes[..length - 1].to_vec()).map_err(|_| "native_string_utf8")
    }
    fn finish(&self) -> Result<()> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err("native_message_shape")
        }
    }
}
/// One instance per real frontend connection. `titles` comes only from trusted
/// native host creation; target-page titles or received context cannot bind it.
pub(crate) struct Observer {
    objects: HashMap<u32, Interface>,
    retired: HashSet<u32>,
    surfaces: HashSet<u32>,
    parents: HashMap<u32, u32>,
    xdg: HashMap<u32, u32>,
    tops: HashMap<u32, u32>,
    subs: HashMap<u32, u32>,
    titles: HashMap<String, String>,
    leases: HashMap<u32, String>,
    identities: HashMap<u32, String>,
    contexts: HashMap<u32, Context>,
    buffers: [Vec<u8>; 2],
    latest: HashMap<u32, Snapshot>,
    invalidated: HashSet<u32>,
    disabled: bool,
    overflow: bool,
}
impl Observer {
    pub(crate) fn new(titles: HashMap<String, String>) -> Result<Self> {
        Self::validate_titles(&titles)?;
        Ok(Self {
            objects: HashMap::from([(1, Interface::Display)]),
            retired: HashSet::new(),
            surfaces: HashSet::new(),
            parents: HashMap::new(),
            xdg: HashMap::new(),
            tops: HashMap::new(),
            subs: HashMap::new(),
            titles,
            leases: HashMap::new(),
            identities: HashMap::new(),
            contexts: HashMap::new(),
            buffers: [Vec::new(), Vec::new()],
            latest: HashMap::new(),
            invalidated: HashSet::new(),
            disabled: false,
            overflow: false,
        })
    }
    fn validate_titles(titles: &HashMap<String, String>) -> Result<()> {
        if titles.len() > 32
            || titles.iter().any(|(title, lease)| {
                title.is_empty() || lease.is_empty() || title.len() > 256 || lease.len() > 256
            })
            || titles.values().collect::<HashSet<_>>().len() != titles.len()
        {
            return Err("native_expected_identity_budget");
        }
        Ok(())
    }
    pub(crate) fn expect(&mut self, titles: HashMap<String, String>) -> Result<()> {
        Self::validate_titles(&titles)?;
        let removed: Vec<u32> = self
            .identities
            .iter()
            .filter(|(_, title)| self.titles.get(*title) != titles.get(*title))
            .map(|(surface, _)| *surface)
            .collect();
        self.titles = titles;
        for surface in removed {
            self.invalidate_surface(surface, false)?;
            self.leases.remove(&surface);
            self.identities.remove(&surface);
        }
        Ok(())
    }
    fn add(&mut self, id: u32, interface: Interface) -> Result<()> {
        if !(2..0xff000000).contains(&id)
            || self.objects.contains_key(&id)
            || self.retired.contains(&id)
        {
            return Err("native_object_identity");
        }
        if self.objects.len() + self.retired.len() >= OBJECT_LIMIT {
            return Err("native_object_budget");
        }
        self.objects.insert(id, interface);
        Ok(())
    }
    fn root(&self, mut surface: u32) -> Result<u32> {
        let mut seen = HashSet::new();
        while let Some(parent) = self.parents.get(&surface) {
            if !seen.insert(surface) {
                return Err("native_surface_cycle");
            }
            surface = *parent;
        }
        Ok(surface)
    }
    fn invalidate(&mut self, id: u32) {
        self.latest.remove(&id);
        // At most one invalidation/current snapshot per live object, no history.
        if self.contexts.contains_key(&id) {
            if self.invalidated.len() >= OBJECT_LIMIT && !self.invalidated.contains(&id) {
                self.overflow = true;
            } else {
                self.invalidated.insert(id);
            }
        }
    }
    fn reset(&mut self, id: u32, forget_surface: bool) {
        self.invalidate(id);
        if let Some(context) = self.contexts.get_mut(&id) {
            context.current = State::default();
            context.pending = State::default();
            context.sent = None;
            if forget_surface {
                context.surface = None;
                context.enabled = false;
            }
        }
    }
    fn invalidate_surface(&mut self, surface: u32, forget: bool) -> Result<()> {
        let mut ids = Vec::new();
        for (&id, context) in &self.contexts {
            if let Some(value) = context.surface {
                if value == surface || self.root(value)? == surface {
                    ids.push(id);
                }
            }
        }
        for id in ids {
            self.reset(id, forget);
        }
        Ok(())
    }
    fn forget(&mut self, id: u32, released: bool) -> Result<()> {
        let interface = self.objects.remove(&id);
        if released {
            self.retired.remove(&id);
        } else if interface.is_some() {
            self.retired.insert(id);
        }
        match interface {
            Some(Interface::Text) => {
                self.invalidate(id);
                self.contexts.remove(&id);
            }
            Some(Interface::Surface) => {
                self.invalidate_surface(id, true)?;
                self.surfaces.remove(&id);
                self.xdg.retain(|_, surface| *surface != id);
                self.tops.retain(|_, surface| *surface != id);
                self.subs.retain(|_, surface| *surface != id);
                self.parents
                    .retain(|surface, parent| *surface != id && *parent != id);
                self.leases.remove(&id);
                self.identities.remove(&id);
            }
            Some(Interface::Subsurface) => {
                if let Some(surface) = self.subs.remove(&id) {
                    self.invalidate_surface(surface, false)?;
                    self.parents.remove(&surface);
                }
            }
            Some(Interface::Toplevel) => {
                if let Some(surface) = self.tops.remove(&id) {
                    self.invalidate_surface(surface, false)?;
                    self.leases.remove(&surface);
                    self.identities.remove(&surface);
                }
            }
            Some(Interface::XdgSurface) => {
                if let Some(surface) = self.xdg.remove(&id) {
                    self.invalidate_surface(surface, false)?;
                    self.leases.remove(&surface);
                    self.identities.remove(&surface);
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn close(&mut self) {
        if self.disabled {
            return;
        }
        self.disabled = true;
        self.invalidated.clear();
        for id in self.contexts.keys() {
            self.invalidated.insert(*id);
        }
        self.latest.clear();
        self.contexts.clear();
        self.objects.clear();
        self.retired.clear();
        self.surfaces.clear();
        self.parents.clear();
        self.xdg.clear();
        self.tops.clear();
        self.subs.clear();
        self.leases.clear();
        self.identities.clear();
        self.buffers = [Vec::new(), Vec::new()];
    }
    pub(crate) fn take(&mut self) -> (HashSet<u32>, HashMap<u32, Snapshot>) {
        (
            std::mem::take(&mut self.invalidated),
            std::mem::take(&mut self.latest),
        )
    }
    /// Direction 0 is engine requests; 1 is compositor events. This receives
    /// only positive, bounded socket reads after authentication and FD admission.
    pub(crate) fn feed(&mut self, direction: usize, bytes: &[u8]) -> Result<()> {
        if self.disabled {
            return Ok(());
        }
        let result = self.feed_inner(direction, bytes);
        if result.is_err() {
            self.close();
        }
        result
    }
    fn feed_inner(&mut self, direction: usize, bytes: &[u8]) -> Result<()> {
        if direction > 1 || bytes.len() > 65536 {
            return Err("native_read_budget");
        }
        self.buffers[direction].extend_from_slice(bytes);
        let mut at = 0;
        while self.buffers[direction].len() - at >= 8 {
            let header = &self.buffers[direction][at..at + 8];
            let id = u32::from_ne_bytes(header[..4].try_into().map_err(|_| "native_header_shape")?);
            let word =
                u32::from_ne_bytes(header[4..].try_into().map_err(|_| "native_header_shape")?);
            let size = (word >> 16) as usize;
            let opcode = (word & 65535) as u16;
            if !(8..=MESSAGE_LIMIT).contains(&size) || !size.is_multiple_of(4) {
                return Err("native_header_shape");
            }
            if self.buffers[direction].len() - at < size {
                break;
            }
            let payload = self.buffers[direction][at + 8..at + size].to_vec();
            at += size;
            self.message(direction, id, opcode, &payload)?;
            if self.disabled {
                return Ok(());
            }
            if self.overflow {
                return Err("native_invalidation_budget");
            }
        }
        self.buffers[direction].drain(..at);
        if self.buffers[direction].len() > MESSAGE_LIMIT {
            return Err("native_incomplete_budget");
        }
        Ok(())
    }
    fn message(&mut self, direction: usize, id: u32, opcode: u16, payload: &[u8]) -> Result<()> {
        let interface = self.objects.get(&id).copied();
        let mut r = Reader {
            bytes: payload,
            at: 0,
        };
        if direction == 1 {
            match (interface, opcode) {
                (Some(Interface::Display), 1) => {
                    let forgotten = r.word()?;
                    if forgotten <= 1 {
                        return Err("native_server_object_identity");
                    }
                    self.forget(forgotten, true)?;
                }
                (Some(Interface::Display), 0) => {
                    self.close();
                    return Ok(());
                }
                (Some(Interface::Text), _) => return self.text_server(id, opcode, &mut r),
                _ => return Ok(()),
            }
            return r.finish();
        }
        match (interface, opcode) {
            (Some(Interface::Display), 1) => {
                self.add(r.word()?, Interface::Registry)?;
            }
            (Some(Interface::Registry), 0) => {
                r.word()?;
                let name = r.string()?;
                let version = r.word()?;
                let new = r.word()?;
                let kind = match name.as_str() {
                    "wl_compositor" => Some(Interface::Compositor),
                    "wl_subcompositor" => Some(Interface::Subcompositor),
                    "xdg_wm_base" => Some(Interface::Shell),
                    "zwp_text_input_manager_v3" => {
                        if version != 1 {
                            return Err("native_text_version");
                        }
                        Some(Interface::TextManager)
                    }
                    _ => None,
                };
                if let Some(kind) = kind {
                    self.add(new, kind)?;
                }
            }
            (Some(Interface::Compositor), 0) => {
                let new = r.word()?;
                self.add(new, Interface::Surface)?;
                self.surfaces.insert(new);
            }
            (Some(Interface::Subcompositor), 1) => {
                let new = r.word()?;
                let surface = r.word()?;
                let parent = r.word()?;
                if !self.surfaces.contains(&surface) || !self.surfaces.contains(&parent) {
                    return Err("native_subsurface_identity");
                }
                self.add(new, Interface::Subsurface)?;
                self.invalidate_surface(surface, false)?;
                self.subs.insert(new, surface);
                self.parents.insert(surface, parent);
                self.root(surface)?;
            }
            (Some(Interface::Shell), 2) => {
                let new = r.word()?;
                let surface = r.word()?;
                if !self.surfaces.contains(&surface) {
                    return Err("native_xdg_identity");
                }
                self.add(new, Interface::XdgSurface)?;
                self.xdg.insert(new, surface);
            }
            (Some(Interface::XdgSurface), 1) => {
                let new = r.word()?;
                let surface = *self.xdg.get(&id).ok_or("native_toplevel_identity")?;
                if !self.surfaces.contains(&surface) {
                    return Err("native_toplevel_identity");
                }
                self.add(new, Interface::Toplevel)?;
                self.tops.insert(new, surface);
            }
            (Some(Interface::Toplevel), 2) => {
                let title = r.string()?;
                let surface = *self.tops.get(&id).ok_or("native_toplevel_identity")?;
                let old = self.identities.remove(&surface);
                self.leases.remove(&surface);
                let identity = if let Some(lease) = self.titles.get(&title) {
                    self.leases.insert(surface, lease.clone());
                    self.identities.insert(surface, title.clone());
                    Some(title)
                } else {
                    None
                };
                if old != identity {
                    self.invalidate_surface(surface, false)?;
                }
            }
            (Some(Interface::TextManager), 1) => {
                let new = r.word()?;
                r.word()?;
                self.add(new, Interface::Text)?;
                self.contexts.insert(new, Context::default());
            }
            (Some(Interface::Text), _) => return self.text_client(id, opcode, &mut r),
            (
                Some(
                    Interface::Surface
                    | Interface::Subsurface
                    | Interface::XdgSurface
                    | Interface::Toplevel
                    | Interface::Shell
                    | Interface::Subcompositor
                    | Interface::TextManager,
                ),
                0,
            ) => self.forget(id, false)?,
            _ => return Ok(()),
        }
        r.finish()
    }
    fn text_client(&mut self, id: u32, opcode: u16, r: &mut Reader<'_>) -> Result<()> {
        if opcode == 0 {
            self.forget(id, false)?;
            return r.finish();
        }
        if opcode == 1 || opcode == 2 {
            self.reset(id, false);
            self.contexts
                .get_mut(&id)
                .ok_or("native_context_identity")?
                .enabled = opcode == 1;
            return r.finish();
        }
        let context = self
            .contexts
            .get_mut(&id)
            .ok_or("native_context_identity")?;
        match opcode {
            3 => {
                let text = r.string()?;
                let cursor = r.word()?;
                let anchor = r.word()?;
                if text.len() > 4000
                    || !text.is_char_boundary(cursor as usize)
                    || !text.is_char_boundary(anchor as usize)
                {
                    return Err("native_surrounding_range");
                }
                context.pending.surrounding = Some((text, cursor, anchor));
            }
            4 => {
                r.word()?;
            }
            5 => {
                let hints = r.word()?;
                let purpose = r.word()?;
                if hints > 1023 || purpose > 13 {
                    return Err("native_content_type");
                }
                context.pending.content = Some((hints, purpose));
            }
            6 => {
                let rect = [
                    r.word()? as i32,
                    r.word()? as i32,
                    r.word()? as i32,
                    r.word()? as i32,
                ];
                if rect[2] < 0 || rect[3] < 0 {
                    return Err("native_caret_range");
                }
                context.pending.caret = Some(rect);
            }
            7 => {
                context.count = context.count.wrapping_add(1);
                context.current.merge(std::mem::take(&mut context.pending));
                context.sent = Some((context.count, context.surface, context.current.clone()));
            }
            _ => return Err("native_text_request"),
        }
        r.finish()
    }
    fn text_server(&mut self, id: u32, opcode: u16, r: &mut Reader<'_>) -> Result<()> {
        match opcode {
            0 => {
                let surface = r.word()?;
                if !self.surfaces.contains(&surface) {
                    return Err("native_enter_surface");
                }
                if self
                    .contexts
                    .get(&id)
                    .ok_or("native_context_identity")?
                    .surface
                    != Some(surface)
                {
                    self.reset(id, false);
                    self.contexts
                        .get_mut(&id)
                        .ok_or("native_context_identity")?
                        .surface = Some(surface);
                }
            }
            1 => {
                let surface = r.word()?;
                if self
                    .contexts
                    .get(&id)
                    .ok_or("native_context_identity")?
                    .surface
                    == Some(surface)
                {
                    self.reset(id, true);
                }
            }
            5 => {
                let serial = r.word()?;
                let context = self.contexts.get(&id).ok_or("native_context_identity")?;
                if let Some((sent, surface, state)) = &context.sent {
                    if *sent == serial && context.enabled && *surface == context.surface {
                        if let Some(surface) = surface {
                            let root = self.root(*surface)?;
                            if let Some(lease) = self.leases.get(&root) {
                                self.latest.insert(
                                    id,
                                    Snapshot {
                                        object: id,
                                        surface: *surface,
                                        root,
                                        lease: lease.clone(),
                                        serial,
                                        state: state.clone(),
                                    },
                                );
                            }
                        }
                    }
                }
            }
            _ => return Ok(()),
        }
        r.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message(id: u32, opcode: u16, payload: &[u8]) -> Vec<u8> {
        let mut bytes = id.to_ne_bytes().to_vec();
        bytes.extend_from_slice(
            &(((payload.len() as u32 + 8) << 16) | u32::from(opcode)).to_ne_bytes(),
        );
        bytes.extend_from_slice(payload);
        bytes
    }
    fn words(values: &[u32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_ne_bytes())
            .collect()
    }
    fn text(value: &str) -> Vec<u8> {
        let mut bytes = (value.len() as u32 + 1).to_ne_bytes().to_vec();
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        bytes
    }
    fn observer() -> Observer {
        let mut value = Observer::new(HashMap::from([(
            "trusted-window".into(),
            "owned-lease".into(),
        )]))
        .unwrap();
        // The independent native-marshaller proof must cover object creation;
        // these unit fixtures isolate current-state and lifetime behavior.
        value.add(2, Interface::Surface).unwrap();
        value.surfaces.insert(2);
        value.add(3, Interface::Toplevel).unwrap();
        value.tops.insert(3, 2);
        value.add(4, Interface::Text).unwrap();
        value.contexts.insert(4, Context::default());
        value
            .feed(0, &message(3, 2, &text("trusted-window")))
            .unwrap();
        value.feed(1, &message(4, 0, &words(&[2]))).unwrap();
        value.take();
        value
    }
    fn state() -> Vec<u8> {
        let mut surrounding = text("A😀日本ZQ");
        surrounding.extend(words(&[13, 13]));
        [
            message(4, 1, &[]),
            message(4, 3, &surrounding),
            message(4, 6, &words(&[u32::MAX - 2, 17, 0, 18])),
            message(4, 7, &[]),
        ]
        .concat()
    }
    #[test]
    fn fragmented_unicode_waits_for_exact_native_done() {
        let input = state();
        for split in 0..=input.len() {
            let mut value = observer();
            value.feed(0, &input[..split]).unwrap();
            value.feed(0, &input[split..]).unwrap();
            assert!(value.take().1.is_empty());
            value.feed(1, &message(4, 5, &words(&[0]))).unwrap();
            assert!(value.take().1.is_empty());
            value.feed(1, &message(4, 5, &words(&[1]))).unwrap();
            let (_, snapshots) = value.take();
            let snapshot = &snapshots[&4];
            assert_eq!(snapshot.state.caret, Some([-3, 17, 0, 18]));
            assert_eq!(
                snapshot.state.surrounding,
                Some(("A😀日本ZQ".into(), 13, 13))
            );
            assert_eq!(
                (
                    snapshot.object,
                    snapshot.surface,
                    snapshot.root,
                    snapshot.serial
                ),
                (4, 2, 2, 1)
            );
            assert_eq!(snapshot.lease, "owned-lease");
        }
    }
    #[test]
    fn unknown_surrounding_does_not_become_empty_or_old_text() {
        let mut value = observer();
        value.feed(0, &state()).unwrap();
        value.feed(1, &message(4, 5, &words(&[1]))).unwrap();
        value.take();
        value
            .feed(
                0,
                &[
                    message(4, 1, &[]),
                    message(4, 6, &words(&[1, 2, 0, 3])),
                    message(4, 7, &[]),
                ]
                .concat(),
            )
            .unwrap();
        value.feed(1, &message(4, 5, &words(&[2]))).unwrap();
        assert!(value.take().1[&4].state.surrounding.is_none());
        let mut empty = text("");
        empty.extend(words(&[0, 0]));
        value
            .feed(0, &[message(4, 3, &empty), message(4, 7, &[])].concat())
            .unwrap();
        value.feed(1, &message(4, 5, &words(&[3]))).unwrap();
        assert_eq!(
            value.take().1[&4].state.surrounding,
            Some((String::new(), 0, 0))
        );
    }
    #[test]
    fn foreign_title_leave_and_close_remove_queryable_state() {
        let mut value = observer();
        value.feed(0, &state()).unwrap();
        value
            .feed(0, &message(3, 2, &text("target-page-title")))
            .unwrap();
        value.feed(1, &message(4, 5, &words(&[1]))).unwrap();
        let (invalidated, snapshots) = value.take();
        assert!(invalidated.contains(&4));
        assert!(snapshots.is_empty());
        value
            .feed(0, &message(3, 2, &text("trusted-window")))
            .unwrap();
        value.feed(0, &state()).unwrap();
        value.feed(1, &message(4, 1, &words(&[2]))).unwrap();
        value.feed(1, &message(4, 5, &words(&[2]))).unwrap();
        assert!(value.take().1.is_empty());
        value.close();
        value.feed(0, &state()).unwrap();
        assert!(value.contexts.is_empty());
        assert!(value.take().1.is_empty());
    }
    #[test]
    fn invalid_unicode_boundary_retires_observation() {
        let mut value = observer();
        let mut malformed = text("A😀日本ZQ");
        malformed.extend(words(&[2, 13]));
        assert!(value.feed(0, &message(4, 3, &malformed)).is_err());
        assert!(value.disabled);
        assert!(value.contexts.is_empty());
        assert!(value.take().1.is_empty());
    }
    #[test]
    fn object_reuse_requires_real_server_release() {
        let mut value = observer();
        value.feed(0, &message(2, 0, &[])).unwrap();
        assert!(value.add(2, Interface::Surface).is_err());
        value.feed(1, &message(1, 1, &words(&[2]))).unwrap();
        assert!(value.add(2, Interface::Surface).is_ok());
        assert!(!value.tops.contains_key(&3));
    }
    #[test]
    fn opaque_protocol_does_not_change_or_bind_context() {
        let mut value = observer();
        value
            .feed(0, &message(424242, 77, &[255, 0, 1, 2]))
            .unwrap();
        value.feed(0, &state()).unwrap();
        value.feed(1, &message(4, 5, &words(&[1]))).unwrap();
        assert_eq!(value.take().1.len(), 1);
    }
}
