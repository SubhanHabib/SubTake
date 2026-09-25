use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill,
    point, prelude::*, px, relative, size,
};
use subtake_theme::Theme;

actions!(
    text_input,
    [
        Accept,
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Cancel,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        SelectToStart,
        SelectToEnd,
        DeleteWordLeft,
        DeleteWordRight,
        DeleteToStart,
        DeleteToEnd,
        Undo,
        Redo,
        Up,
        Down,
    ]
);

/// The field as it stood before an edit, for undo.
struct Snapshot {
    content: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
}

/// Whether `c` belongs to a word, for the word-wise moves and deletes and
/// for a double click.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The start of the word before `offset`, skipping the spaces and
/// punctuation between: where ⌥← lands.
fn previous_word(text: &str, offset: usize) -> usize {
    let mut chars = text[..offset].char_indices().rev().peekable();
    while chars.next_if(|(_, c)| !is_word(*c)).is_some() {}
    let mut start = chars.peek().map_or(0, |(i, _)| *i);
    while let Some((i, _)) = chars.next_if(|(_, c)| is_word(*c)) {
        start = i;
    }
    start
}

/// The end of the word after `offset`: where ⌥→ lands.
fn next_word(text: &str, offset: usize) -> usize {
    let rest = &text[offset..];
    let mut chars = rest.char_indices().peekable();
    while chars.next_if(|(_, c)| !is_word(*c)).is_some() {}
    while chars.next_if(|(_, c)| is_word(*c)).is_some() {}
    offset + chars.peek().map_or(rest.len(), |(i, _)| *i)
}

/// The word `offset` sits in, for a double click — or the run of
/// spaces and punctuation, if that is what was clicked.
fn run_at(text: &str, offset: usize) -> Range<usize> {
    let Some(c) = text[offset..]
        .chars()
        .next()
        .or_else(|| text[..offset].chars().next_back())
    else {
        return offset..offset;
    };
    let word = is_word(c);
    let start = text[..offset]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c) == word)
        .last()
        .map_or(offset, |(i, _)| i);
    let rest = &text[offset..];
    let end = offset
        + rest
            .char_indices()
            .find(|(_, c)| is_word(*c) != word)
            .map_or(rest.len(), |(i, _)| i);
    start..end
}

pub struct TextInput {
    pub theme: Theme,
    accepted: Box<dyn Fn(String, &mut Window, &mut App)>,
    /// Fires on every edit rather than on commit — for a field whose value
    /// is consumed as it is typed (a palette filter) rather than accepted.
    changed: Option<Box<dyn Fn(String, &mut Window, &mut App)>>,
    /// Fires when the field is dismissed with escape. A field inside a
    /// transient surface has to hand the key on: `cancel` stops propagation,
    /// so the surface never sees the escape that was meant to close it.
    cancelled: Option<Box<dyn Fn(&mut Window, &mut App)>>,
    /// Takes the up and down arrows, for a field that steers a list below
    /// it (the palette's filter). Without one they go to the start and end,
    /// as they do in any single-line field.
    stepped: Option<Box<dyn Fn(isize, &mut Window, &mut App)>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// The last edit was a typed character, so the next one joins its undo
    /// step: undo takes back a word's typing, not one letter.
    typing: bool,
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    committed: String,
    blur_observer: Option<gpui::Subscription>,
    scroll_offset: Pixels,
    /// A glyph leading the text and a key cap ending it: what a filter
    /// field shows of what it filters and how to leave it.
    leading: Option<SharedString>,
    trailing: Option<SharedString>,
}

impl TextInput {
    pub fn set_handler(&mut self, accepted: impl Fn(String, &mut Window, &mut App) + 'static) {
        self.accepted = Box::new(accepted);
    }

    pub fn set_on_change(&mut self, changed: impl Fn(String, &mut Window, &mut App) + 'static) {
        self.changed = Some(Box::new(changed));
    }

    pub fn set_on_cancel(&mut self, cancelled: impl Fn(&mut Window, &mut App) + 'static) {
        self.cancelled = Some(Box::new(cancelled));
    }

    pub fn set_on_step(&mut self, stepped: impl Fn(isize, &mut Window, &mut App) + 'static) {
        self.stepped = Some(Box::new(stepped));
    }
    /// Replace the contents outright, ignoring focus — `sync` deliberately
    /// leaves a focused field alone, which is wrong when the caller is
    /// reopening the surface the field lives in.
    pub fn reset(&mut self, value: &str) {
        self.content = value.to_owned().into();
        self.selected_range = self.content.len()..self.content.len();
        self.committed = value.to_owned();
        self.forget();
    }

    /// Drop the undo history: the value came from outside, so undoing into
    /// what was there before would bring back somebody else's text.
    fn forget(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.typing = false;
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<SharedString>) {
        self.placeholder = placeholder.into();
    }

    /// Lead the text with the glyph `name`, at `muted`.
    pub fn set_leading(&mut self, name: impl Into<SharedString>) {
        self.leading = Some(name.into());
    }

    /// End the field with a key cap reading `key`: the key that leaves it.
    pub fn set_trailing(&mut self, key: impl Into<SharedString>) {
        self.trailing = Some(key.into());
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus_handle, cx);
    }

    pub fn new(
        cx: &mut Context<Self>,
        content: String,
        theme: Theme,
        accepted: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            theme,
            accepted: Box::new(accepted),
            changed: None,
            cancelled: None,
            stepped: None,
            undo: Vec::new(),
            redo: Vec::new(),
            typing: false,
            focus_handle: cx.focus_handle(),
            committed: content.clone(),
            content: content.into(),
            placeholder: "".into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            blur_observer: None,
            scroll_offset: px(0.),
            leading: None,
            trailing: None,
        }
    }

    pub fn sync(&mut self, value: &str, window: &Window) {
        if !self.focus_handle.is_focused(window) && self.content.as_ref() != value {
            self.content = value.to_owned().into();
            self.selected_range = self.content.len()..self.content.len();
            self.committed = value.to_owned();
            self.forget();
        }
    }

    fn accept(&mut self, _: &Accept, window: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_none() {
            self.commit(window, cx);
            cx.stop_propagation();
        }
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.committed != self.content.as_ref() {
            self.committed = self.content.to_string();
            (self.accepted)(self.committed.clone(), window, cx);
        }
    }

    fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        self.content = self.committed.clone().into();
        self.selected_range = self.content.len()..self.content.len();
        self.marked_range = None;
        if let Some(cancelled) = self.cancelled.take() {
            cancelled(window, cx);
            self.cancelled = Some(cancelled);
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(previous_word(&self.content, self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(next_word(&self.content, self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(previous_word(&self.content, self.cursor_offset()), cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(next_word(&self.content, self.cursor_offset()), cx);
    }

    fn select_to_start(&mut self, _: &SelectToStart, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, cx);
    }

    fn select_to_end(&mut self, _: &SelectToEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.content.len(), cx);
    }

    /// Delete from the caret to `offset`, or the selection if there is one.
    fn delete_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(offset, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, w: &mut Window, cx: &mut Context<Self>) {
        self.delete_to(previous_word(&self.content, self.cursor_offset()), w, cx)
    }

    fn delete_word_right(&mut self, _: &DeleteWordRight, w: &mut Window, cx: &mut Context<Self>) {
        self.delete_to(next_word(&self.content, self.cursor_offset()), w, cx)
    }

    fn delete_to_start(&mut self, _: &DeleteToStart, w: &mut Window, cx: &mut Context<Self>) {
        self.delete_to(0, w, cx)
    }

    fn delete_to_end(&mut self, _: &DeleteToEnd, w: &mut Window, cx: &mut Context<Self>) {
        self.delete_to(self.content.len(), w, cx)
    }

    fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        match self.stepped.take() {
            Some(stepped) => {
                stepped(-1, window, cx);
                self.stepped = Some(stepped);
            }
            None => self.move_to(0, cx),
        }
    }

    fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        match self.stepped.take() {
            Some(stepped) => {
                stepped(1, window, cx);
                self.stepped = Some(stepped);
            }
            None => self.move_to(self.content.len(), cx),
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            content: self.content.clone(),
            selected_range: self.selected_range.clone(),
            selection_reversed: self.selection_reversed,
        }
    }

    /// Put `snapshot` back, and tell a live listener the text changed.
    fn restore(&mut self, snapshot: Snapshot, window: &mut Window, cx: &mut Context<Self>) {
        self.content = snapshot.content;
        self.selected_range = snapshot.selected_range;
        self.selection_reversed = snapshot.selection_reversed;
        self.marked_range = None;
        self.typing = false;
        if let Some(changed) = self.changed.take() {
            changed(self.content.to_string(), window, cx);
            self.changed = Some(changed);
        }
        cx.notify();
    }

    fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(snapshot) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(snapshot, window, cx);
        }
    }

    fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(snapshot) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(snapshot, window, cx);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        cx.stop_propagation();
        self.is_selecting = true;

        let index = self.index_for_mouse_position(event.position);
        match event.click_count {
            // A double click takes the word under the pointer, a triple
            // click the whole line — which, in a one-line field, is all of it.
            2 => {
                let run = run_at(&self.content, index);
                self.move_to(run.start, cx);
                self.select_to(run.end, cx);
                self.is_selecting = false;
            }
            n if n >= 3 => {
                self.move_to(0, cx);
                self.select_to(self.content.len(), cx);
                self.is_selecting = false;
            }
            _ if event.modifiers.shift => self.select_to(index, cx),
            _ => self.move_to(index, cx),
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace("\n", " "), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.typing = false;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left() + self.scroll_offset)
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.typing = false;
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .char_indices()
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .char_indices()
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        // A run of typing is one undo step; a paste, a delete or typing
        // over a selection starts a new one.
        let typing = range.is_empty() && new_text.chars().count() == 1;
        if !(typing && self.typing) && (!range.is_empty() || !new_text.is_empty()) {
            self.undo.push(self.snapshot());
            self.redo.clear();
        }
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.typing = typing;
        self.marked_range.take();
        // Every edit path — typing, paste, backspace, delete — funnels
        // through here, so this is the one place a live listener has to sit.
        if let Some(changed) = self.changed.take() {
            changed(self.content.to_string(), window, cx);
            self.changed = Some(changed);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|r| {
                let offset = |n: usize| {
                    new_text
                        .char_indices()
                        .scan(0, |count, (i, ch)| {
                            let previous = *count;
                            *count += ch.len_utf16();
                            Some((i, previous))
                        })
                        .find(|(_, count)| *count >= n)
                        .map(|(i, _)| i)
                        .unwrap_or(new_text.len())
                };
                range.start + offset(r.start)..range.start + offset(r.end)
            })
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start) - self.scroll_offset,
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end) - self.scroll_offset,
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;

        assert_eq!(last_layout.text, self.content);
        let utf8_index = last_layout.index_for_x(line_point.x + self.scroll_offset)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_offset: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), input.theme.muted)
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let scroll_offset = input
            .scroll_offset
            .max(cursor_pos - bounds.size.width + px(8.))
            .min(cursor_pos)
            .max(px(0.));
        // The caret is the height of the text line, centred in the control,
        // not the control's full 40px: the field's line height is inflated
        // to centre the text, and a caret that tall reads as a divider.
        let caret_height = font_size * 1.3;
        let caret_top = bounds.top() + (bounds.size.height - caret_height) / 2.;
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos - scroll_offset, caret_top),
                        size(px(2.), caret_height),
                    ),
                    input.theme.accent,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start) - scroll_offset,
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end) - scroll_offset,
                            bounds.bottom(),
                        ),
                    ),
                    input.theme.accent_soft,
                )),
                None,
            )
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
            scroll_offset,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        line.paint(
            point(bounds.left() - prepaint.scroll_offset, bounds.top()),
            window.line_height(),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        )
        .unwrap();

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
            input.scroll_offset = prepaint.scroll_offset;
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.blur_observer.is_none() {
            self.blur_observer = Some(cx.on_blur(&self.focus_handle, window, |this, w, cx| {
                this.marked_range = None;
                this.commit(w, cx);
            }));
        }
        // Every field is `text-input`, so its hover wash is keyed off the
        // entity, which is the thing that is unique per field.
        let hover_key = format!("input-{:?}-hover", cx.entity_id());
        let theme = self.theme;
        let focused = self.focus_handle.is_focused(window);
        let leading = self.leading.clone();
        let trailing = self.trailing.clone();
        div()
            .flex()
            .id("text-input")
            .on_hover(crate::motion::hover_listener(hover_key.clone()))
            .key_context("SubTakeInput")
            .on_action(cx.listener(Self::accept))
            .on_action(cx.listener(Self::cancel))
            .track_focus(&self.focus_handle(cx))
            .tab_index(0)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::select_to_start))
            .on_action(cx.listener(Self::select_to_end))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::delete_to_start))
            .on_action(cx.listener(Self::delete_to_end))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .line_height(px(Theme::control_height()))
            .text_size(px(Theme::font_control()))
            .child(
                div()
                    .h(px(Theme::control_height()))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(Theme::icon_gap_row()))
                    .px(px(Theme::input_padding()))
                    .bg(crate::motion::hover_blend(
                        &hover_key,
                        theme.sunk,
                        theme.sunk2,
                    ))
                    .rounded_full()
                    // A field shows its focus however it got it, not only
                    // from the keyboard: the caret alone is a 2px line, and a
                    // clicked field is where the typing is about to go.
                    .when(focused, |el| el.shadow(vec![crate::focus_ring(theme)]))
                    .overflow_hidden()
                    .children(
                        leading
                            .map(|name| crate::icon_sized(&name, Theme::icon_size(), theme.muted)),
                    )
                    // The text takes what the glyph and the cap leave.
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(TextElement { input: cx.entity() }),
                    )
                    .children(trailing.map(|key| crate::unused::key_cap(key, theme))),
            )
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub fn init(cx: &mut App) {
    // One binding set per App, even when editor/launcher/options are all opened.
    struct InputBindings;
    impl gpui::Global for InputBindings {}
    if cx.has_global::<InputBindings>() {
        return;
    }
    cx.set_global(InputBindings);
    // Bundled Geist / Geist Mono, registered before the first frame paints.
    crate::fonts::register(cx);
    // The editing keys every text field on the platform answers to. They
    // are scoped to the field, so ⌘Z here undoes typing rather than the
    // editor's last edit, and ⌘A selects the text rather than the timeline.
    let field = Some("SubTakeInput");
    let mut bindings = vec![
        KeyBinding::new("enter", Accept, field),
        KeyBinding::new("escape", Cancel, field),
        KeyBinding::new("backspace", Backspace, field),
        KeyBinding::new("shift-backspace", Backspace, field),
        KeyBinding::new("delete", Delete, field),
        KeyBinding::new("left", Left, field),
        KeyBinding::new("right", Right, field),
        KeyBinding::new("shift-left", SelectLeft, field),
        KeyBinding::new("shift-right", SelectRight, field),
        KeyBinding::new("up", Up, field),
        KeyBinding::new("down", Down, field),
        KeyBinding::new("shift-up", SelectToStart, field),
        KeyBinding::new("shift-down", SelectToEnd, field),
        KeyBinding::new("home", Home, field),
        KeyBinding::new("end", End, field),
        KeyBinding::new("shift-home", SelectToStart, field),
        KeyBinding::new("shift-end", SelectToEnd, field),
    ];
    #[cfg(target_os = "macos")]
    bindings.extend([
        KeyBinding::new("cmd-a", SelectAll, field),
        KeyBinding::new("cmd-c", Copy, field),
        KeyBinding::new("cmd-x", Cut, field),
        KeyBinding::new("cmd-v", Paste, field),
        KeyBinding::new("cmd-z", Undo, field),
        KeyBinding::new("cmd-shift-z", Redo, field),
        KeyBinding::new("cmd-left", Home, field),
        KeyBinding::new("cmd-right", End, field),
        KeyBinding::new("cmd-up", Home, field),
        KeyBinding::new("cmd-down", End, field),
        KeyBinding::new("cmd-shift-left", SelectToStart, field),
        KeyBinding::new("cmd-shift-right", SelectToEnd, field),
        KeyBinding::new("alt-left", WordLeft, field),
        KeyBinding::new("alt-right", WordRight, field),
        KeyBinding::new("alt-shift-left", SelectWordLeft, field),
        KeyBinding::new("alt-shift-right", SelectWordRight, field),
        KeyBinding::new("alt-backspace", DeleteWordLeft, field),
        KeyBinding::new("alt-delete", DeleteWordRight, field),
        KeyBinding::new("cmd-backspace", DeleteToStart, field),
        KeyBinding::new("cmd-delete", DeleteToEnd, field),
        // The Emacs keys every Cocoa field takes: ⌃A is the start of the
        // line on a Mac, not select all.
        KeyBinding::new("ctrl-a", Home, field),
        KeyBinding::new("ctrl-e", End, field),
        KeyBinding::new("ctrl-b", Left, field),
        KeyBinding::new("ctrl-f", Right, field),
        KeyBinding::new("ctrl-h", Backspace, field),
        KeyBinding::new("ctrl-d", Delete, field),
        KeyBinding::new("ctrl-k", DeleteToEnd, field),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, field),
    ]);
    #[cfg(not(target_os = "macos"))]
    bindings.extend([
        KeyBinding::new("ctrl-a", SelectAll, field),
        KeyBinding::new("ctrl-c", Copy, field),
        KeyBinding::new("ctrl-x", Cut, field),
        KeyBinding::new("ctrl-v", Paste, field),
        KeyBinding::new("shift-insert", Paste, field),
        KeyBinding::new("ctrl-insert", Copy, field),
        KeyBinding::new("shift-delete", Cut, field),
        KeyBinding::new("ctrl-z", Undo, field),
        KeyBinding::new("ctrl-y", Redo, field),
        KeyBinding::new("ctrl-shift-z", Redo, field),
        KeyBinding::new("ctrl-left", WordLeft, field),
        KeyBinding::new("ctrl-right", WordRight, field),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, field),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, field),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, field),
        KeyBinding::new("ctrl-delete", DeleteWordRight, field),
        KeyBinding::new("ctrl-home", Home, field),
        KeyBinding::new("ctrl-end", End, field),
    ]);
    cx.bind_keys(bindings);
}

#[cfg(test)]
mod tests {
    use super::{next_word, previous_word, run_at};

    #[test]
    fn word_moves_skip_the_gap_then_the_word() {
        let text = "Product demo, take 2";
        assert_eq!(previous_word(text, text.len()), 19);
        assert_eq!(previous_word(text, 19), 14);
        assert_eq!(previous_word(text, 13), 8);
        assert_eq!(previous_word(text, 3), 0);
        assert_eq!(previous_word(text, 0), 0);
        assert_eq!(next_word(text, 0), 7);
        assert_eq!(next_word(text, 7), 12);
        assert_eq!(next_word(text, 12), 18);
        assert_eq!(next_word(text, text.len()), text.len());
    }

    #[test]
    fn a_double_click_takes_the_word_or_the_gap_under_it() {
        let text = "Product demo,  take";
        assert_eq!(run_at(text, 2), 0..7);
        assert_eq!(run_at(text, 8), 8..12);
        assert_eq!(run_at(text, 13), 12..15);
        assert_eq!(run_at(text, text.len()), 15..19);
        assert_eq!(run_at("", 0), 0..0);
        assert_eq!(run_at("héllo wörld", 8), 7..13);
    }
}
