#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod model;
mod rig;
mod storage;
#[cfg(test)]
mod tests;

use eframe::egui::{self, Color32, RichText};
use model::{Contact, Kind, Log};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::Duration,
};

#[derive(Clone)]
enum Pending {
    DeleteContact(String),
    DeleteType(String),
    Import(Log),
}

struct RadioLog {
    log: Log,
    storage: Option<storage::Storage>,
    blocked: bool,
    message: String,
    draft: Contact,
    editing: bool,
    search: String,
    type_filter: String,
    exact: Option<(String, String)>,
    pending: Option<Pending>,
    manage: bool,
    viewing: Option<String>,
    new_type: String,
    new_color: usize,
    rename: Option<(String, String)>,
    radio_rx: Option<Receiver<Result<rig::Radio, String>>>,
    radio_status: String,
}

fn color(name: &str) -> Color32 {
    match name {
        "emerald" => Color32::from_rgb(63, 205, 145),
        "violet" => Color32::from_rgb(184, 151, 255),
        "amber" => Color32::from_rgb(240, 190, 70),
        "sky" => Color32::from_rgb(104, 190, 245),
        "rose" => Color32::from_rgb(245, 136, 160),
        "orange" => Color32::from_rgb(247, 159, 84),
        "teal" => Color32::from_rgb(79, 208, 200),
        _ => Color32::from_rgb(174, 187, 202),
    }
}

impl RadioLog {
    fn new(cc: &eframe::CreationContext<'_>, root: PathBuf) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        // One control height for buttons, text inputs, combo boxes, etc.
        style.spacing.interact_size.y = 34.0;
        style.spacing.combo_height = 300.0;
        // Standard text 16px, nothing smaller than 14px.
        use egui::{FontFamily::{Monospace, Proportional}, FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Small, FontId::new(14.0, Proportional)),
            (TextStyle::Body, FontId::new(16.0, Proportional)),
            (TextStyle::Button, FontId::new(16.0, Proportional)),
            (TextStyle::Heading, FontId::new(24.0, Proportional)),
            (TextStyle::Monospace, FontId::new(16.0, Monospace)),
        ]
        .into();
        style.visuals.selection.bg_fill = Color32::from_rgb(37, 111, 152);
        cc.egui_ctx.set_style(style);
        let (storage, log, blocked, message) = match storage::Storage::open(root) {
            Ok(s) => match s.load() {
                Ok(log) => (Some(s), log, false, "Ready.".into()),
                Err(e) => (
                    Some(s),
                    Log::default(),
                    true,
                    format!("Saved data could not be read. Nothing will be overwritten. {e}"),
                ),
            },
            Err(e) => (None, Log::default(), true, e),
        };
        let draft = Contact::fresh(log.types[0].name.clone());
        Self {
            log,
            storage,
            blocked,
            message,
            draft,
            editing: false,
            search: String::new(),
            type_filter: String::new(),
            exact: None,
            pending: None,
            manage: false,
            viewing: None,
            new_type: String::new(),
            new_color: 0,
            rename: None,
            radio_rx: None,
            radio_status: "SDR++ not checked".into(),
        }
    }
    fn commit(&mut self, next: Log) -> bool {
        if self.blocked {
            return false;
        }
        let result = self
            .storage
            .as_ref()
            .ok_or("Storage is unavailable.".into())
            .and_then(|s| s.save(&next));
        match result {
            Ok(()) => {
                self.log = next;
                self.message = "Saved locally. Previous version backed up.".into();
                true
            }
            Err(e) => {
                self.message = format!("Not saved: {e}. Your existing log is unchanged.");
                false
            }
        }
    }
    fn reset(&mut self) {
        let kind = if self.log.types.iter().any(|t| t.name == self.draft.kind) {
            self.draft.kind.clone()
        } else {
            self.log.types[0].name.clone()
        };
        self.draft = Contact::fresh(kind);
        self.editing = false;
    }
    fn save_contact(&mut self) {
        let mut contact = self.draft.clone();
        if let Err(e) = contact.validate() {
            self.message = e;
            return;
        }
        let mut next = self.log.clone();
        if self.editing {
            if let Some(c) = next.contacts.iter_mut().find(|c| c.id == contact.id) {
                *c = contact;
            } else {
                self.message = "This entry no longer exists. Cancel and add a new entry.".into();
                return;
            }
        } else {
            next.contacts.push(contact);
        }
        if self.commit(next) {
            self.reset();
        }
    }
    fn request_radio(&mut self, ctx: &egui::Context) {
        if self.radio_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.radio_rx = Some(rx);
        self.radio_status = "Reading SDR++…".into();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = rig::read_radio("127.0.0.1:4532".parse().unwrap());
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
    fn receive_radio(&mut self) {
        let result = self.radio_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = result {
            self.radio_rx = None;
            match result {
                Ok(r) => {
                    self.draft.frequency = format!("{:.6}", r.mhz)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .into();
                    self.draft.mode = r.mode.clone();
                    self.draft.datetime = model::now();
                    let hint = model::infer_type(r.mhz, &r.mode);
                    if let Some(t) = self
                        .log
                        .types
                        .iter()
                        .find(|t| model::key(&t.name) == model::key(hint))
                    {
                        self.draft.kind = t.name.clone();
                    }
                    self.radio_status =
                        format!("Connected · {} MHz {}", self.draft.frequency, r.mode);
                    self.message=format!("Autofilled. Suggested type: {hint}; change it if needed. Nothing saved until you press Save.");
                }
                Err(e) => {
                    self.radio_status = "Not connected".into();
                    self.message = e;
                }
            }
        }
    }
    fn import(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Radio log JSON", &["json"])
            .pick_file()
        {
            let result = (|| {
                let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
                if meta.len() > 20 * 1024 * 1024 {
                    return Err("Import file exceeds 20 MB.".into());
                }
                Log::parse(&std::fs::read(&path).map_err(|e| e.to_string())?)
            })();
            match result {
                Ok(log) => self.pending = Some(Pending::Import(log)),
                Err(e) => self.message = format!("Import rejected: {e}"),
            }
        }
    }
    fn export(&mut self) {
        let filename = format!("radio-log-{}.json", chrono::Utc::now().format("%Y-%m-%d"));
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_file_name(&filename)
            .save_file()
        {
            // Do not overwrite the live data or its lock through the export dialog.
            if self
                .storage
                .as_ref()
                .is_some_and(|s| path == s.path() || path == s.root.join("radio-log.lock"))
            {
                self.message = "Choose a separate backup filename.".into();
                return;
            }
            let result = serde_json::to_vec_pretty(&self.log)
                .map_err(|e| e.to_string())
                .and_then(|b| storage::atomic_write(&path, &b));
            self.message = match result {
                Ok(()) => format!(
                    "Exported {} entries to {}",
                    self.log.contacts.len(),
                    path.display()
                ),
                Err(e) => format!("Export failed: {e}"),
            };
        }
    }
    fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
        Self::field_suggest(ui, label, value, &[]);
    }
    /// Text field that offers previously entered values matching what is being typed.
    /// Suggestions only help; any value can still be typed.
    fn field_suggest(ui: &mut egui::Ui, label: &str, value: &mut String, known: &[String]) {
        let label = ui.label(RichText::new(label).color(Color32::from_gray(185)));
        ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY))
            .labelled_by(label.id);
        let typed = value.trim().to_lowercase();
        if typed.is_empty() {
            return;
        }
        let matches: Vec<&String> = known
            .iter()
            .filter(|k| k.as_str() != value.trim() && k.to_lowercase().contains(&typed))
            .take(5)
            .collect();
        if matches.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.small("Previously used:");
            for m in matches {
                if ui.button(m.as_str()).clicked() {
                    *value = m.clone();
                }
            }
        });
    }
    /// Distinct non-empty values already used for a field, most-used first.
    fn known_values(&self, get: impl Fn(&Contact) -> &str) -> Vec<String> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for c in &self.log.contacts {
            let v = get(c).trim();
            if v.is_empty() {
                continue;
            }
            match counts.iter_mut().find(|(k, _)| k == v) {
                Some(entry) => entry.1 += 1,
                None => counts.push((v.to_string(), 1)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        counts.into_iter().map(|(k, _)| k).collect()
    }
    fn form(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading(if self.editing {
            "Edit contact"
        } else {
            "Log a contact"
        });
        ui.add_space(4.0);
        ui.label("Type");
        egui::ComboBox::from_id_salt("entry-type")
            .selected_text(&self.draft.kind)
            .width(285.0)
            .show_ui(ui, |ui| {
                for t in &self.log.types {
                    ui.selectable_value(&mut self.draft.kind, t.name.clone(), &t.name);
                }
            });
        Self::field(ui, "Date / time (UTC)", &mut self.draft.datetime);
        ui.horizontal(|ui| {
            ui.small("YYYY-MM-DD HH:MM");
            if ui.button("Set to now").clicked() {
                self.draft.datetime = model::now();
            }
        });
        // A field is hidden only if this type hides it AND the record has nothing in it,
        // so existing data is never concealed.
        let known_callsigns = self.known_values(|c| &c.callsign);
        let known_sources = self.known_values(|c| &c.satellite);
        let known_modes = self.known_values(|c| &c.mode);
        let known_locations = self.known_values(|c| &c.location);
        let kind = self.log.types.iter().find(|t| t.name == self.draft.kind);
        let show = |key: &str, used: bool| used || !kind.is_some_and(|k| k.hides(key));
        let d = &self.draft;
        let show_callsign = show("callsign", !d.callsign.is_empty());
        let show_satellite = show("satellite", !d.satellite.is_empty());
        let show_freq = show("frequency", !d.frequency.is_empty());
        let show_mode = show("mode", !d.mode.is_empty());
        let show_rst = show("rst", !d.rst_sent.is_empty() || !d.rst_rcvd.is_empty());
        let show_location = show("location", !d.location.is_empty());
        let show_recording = show("recording", !d.recording.is_empty());
        let show_notes = show("notes", !d.notes.is_empty());
        if show_callsign {
            Self::field_suggest(
                ui,
                "Callsign / station",
                &mut self.draft.callsign,
                &known_callsigns,
            );
        }
        if show_satellite {
            Self::field_suggest(
                ui,
                "Satellite / source",
                &mut self.draft.satellite,
                &known_sources,
            );
        }
        if show_freq || show_mode {
            ui.horizontal(|ui| {
                if show_freq {
                    ui.vertical(|ui| {
                        ui.set_width(140.0);
                        Self::field(ui, "Frequency (MHz)", &mut self.draft.frequency);
                    });
                }
                if show_mode {
                    ui.vertical(|ui| {
                        ui.set_width(140.0);
                        Self::field_suggest(ui, "Mode", &mut self.draft.mode, &known_modes);
                    });
                }
            });
        }
        if show_rst {
            ui.horizontal(|ui|{ui.vertical(|ui|{ui.set_width(140.0);Self::field(ui,"RST sent",&mut self.draft.rst_sent);});ui.vertical(|ui|{ui.set_width(140.0);Self::field(ui,"RST received",&mut self.draft.rst_rcvd);});}).response.on_hover_text("Readability, Signal strength, Tone. Voice usually uses two digits, e.g. 59. Leave blank for listening-only entries.");
        }
        if show_location {
            Self::field_suggest(
                ui,
                "Station location / grid",
                &mut self.draft.location,
                &known_locations,
            );
        }
        if show_recording {
            Self::field(ui, "Recording file / link", &mut self.draft.recording);
        }
        if show_notes {
            ui.label("Notes");
            ui.add(
                egui::TextEdit::multiline(&mut self.draft.notes)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .button(if self.editing {
                    "Save changes"
                } else {
                    "Add contact"
                })
                .clicked()
            {
                self.save_contact();
            }
            if self.editing && ui.button("Cancel edit").clicked() {
                self.reset();
            }
        });
        ui.separator();
        if show_freq || show_mode {
            if ui
                .add_enabled(
                    self.radio_rx.is_none(),
                    egui::Button::new("Autofill from SDR++"),
                )
                .clicked()
            {
                self.request_radio(ctx);
            }
            ui.small(&self.radio_status);
        }
        ui.collapsing("SDR++ setup", |ui| {
            ui.label("In SDR++, open Rigctl Server:");
            ui.label("Host: localhost   Port: 4532");
            ui.label("Controlled VFO: Radio");
            ui.label("Click Start, then use Autofill.");
            ui.small("Optional: Listen on startup starts rigctl when SDR++ opens.");
        });
    }
    fn entries(&mut self, ui: &mut egui::Ui) {
        let unique = self
            .log
            .contacts
            .iter()
            .map(|c| model::key(&c.callsign))
            .filter(|s| !s.is_empty())
            .collect::<HashSet<_>>()
            .len();
        ui.horizontal(|ui| {
            ui.heading("Your log");
            ui.label(format!(
                "{} contacts  ·  {} stations",
                self.log.contacts.len(),
                unique
            ));
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(self.type_filter.is_empty(), "All types")
                .clicked()
            {
                self.type_filter.clear();
                self.exact = None;
            }
            for t in &self.log.types {
                let count = self
                    .log
                    .contacts
                    .iter()
                    .filter(|c| c.kind == t.name)
                    .count();
                if ui
                    .selectable_label(
                        self.type_filter == t.name,
                        RichText::new(format!("{} {count}", t.name)).color(color(&t.color)),
                    )
                    .clicked()
                {
                    self.type_filter = t.name.clone();
                    if self.exact.as_ref().is_some_and(|(f, _)| f == "Type") {
                        self.exact = None;
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Search");
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(250.0)
                    .hint_text("Callsign, source, notes…"),
            );
            if ui.button("Show all entries").clicked() {
                self.search.clear();
                self.type_filter.clear();
                self.exact = None;
            }
        });
        if let Some((f, v)) = &self.exact {
            ui.label(RichText::new(format!("{f}: {v}")).color(color("sky")));
        }
        ui.separator();
        let query = self.search.to_lowercase();
        let mut rows: Vec<Contact> = self
            .log
            .contacts
            .iter()
            .filter(|c| {
                (self.type_filter.is_empty() || c.kind == self.type_filter)
                    && self.exact.as_ref().is_none_or(|(f, v)| c.matches(f, v))
                    && (query.is_empty() || c.searchable().contains(&query))
            })
            .cloned()
            .collect();
        rows.sort_by(|a, b| b.datetime.cmp(&a.datetime));
        ui.small(format!("{} shown · click a value to filter", rows.len()));
        if rows.is_empty() {
            ui.add_space(30.0);
            ui.label("No contacts to show.");
            if self.log.contacts.is_empty() {
                ui.label("Add your first entry, or import a JSON log.");
            }
            return;
        }
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("contacts-table")
                    .striped(true)
                    .spacing([18.0, 12.0])
                    .min_col_width(60.0)
                    .show(ui, |ui| {
                        for head in [
                            "Date (UTC)",
                            "Type",
                            "Callsign",
                            "Source",
                            "MHz",
                            "Mode",
                            "RST",
                            "Station location",
                            "Notes / recording",
                            "Actions",
                        ] {
                            ui.strong(head);
                        }
                        ui.end_row();
                        for c in rows {
                            ui.label(c.datetime.replace('T', " "));
                            for f in ["Type", "Callsign", "Source", "Frequency", "Mode"] {
                                let value = c.field(f);
                                if value.trim().is_empty() {
                                    ui.label("—");
                                } else if ui.link(value).clicked() {
                                    self.exact = Some((f.into(), value.into()));
                                    self.type_filter.clear();
                                    self.search.clear();
                                }
                            }
                            ui.label(match (c.rst_sent.is_empty(), c.rst_rcvd.is_empty()) {
                                (false, false) => format!("{} / {}", c.rst_sent, c.rst_rcvd),
                                (false, true) => format!("{} (sent)", c.rst_sent),
                                (true, false) => format!("{} (received)", c.rst_rcvd),
                                (true, true) => String::new(),
                            });
                            if c.location.trim().is_empty() {
                                ui.label("—");
                            } else if ui.link(&c.location).clicked() {
                                self.exact = Some(("Location".into(), c.location.clone()));
                                self.type_filter.clear();
                                self.search.clear();
                            }
                            let detail = if c.recording.is_empty() {
                                c.notes.clone()
                            } else {
                                format!("{}\nRecording: {}", c.notes, c.recording)
                            };
                            ui.add(egui::Label::new(&detail).truncate())
                                .on_hover_text(&detail);
                            ui.horizontal(|ui| {
                                if ui.button("View").clicked() {
                                    self.viewing = Some(c.id.clone());
                                }
                                if ui.button("Edit").clicked() {
                                    self.draft = c.clone();
                                    self.editing = true;
                                }
                                if ui.button("Delete").clicked() {
                                    self.pending = Some(Pending::DeleteContact(c.id.clone()));
                                }
                            });
                            ui.end_row();
                        }
                    });
            });
    }
    fn view_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.viewing.clone() else {
            return;
        };
        let Some(c) = self.log.contacts.iter().find(|c| c.id == id).cloned() else {
            self.viewing = None;
            return;
        };
        let mut open = true;
        let mut edit = false;
        egui::Window::new("Contact details")
            .open(&mut open)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                let kind_color = self
                    .log
                    .types
                    .iter()
                    .find(|t| t.name == c.kind)
                    .map(|t| color(&t.color))
                    .unwrap_or(Color32::GRAY);
                ui.label(RichText::new(&c.kind).heading().color(kind_color));
                ui.add_space(4.0);
                egui::Grid::new("view-grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        let datetime = c.datetime.replace('T', " ");
                        let kind = self.log.types.iter().find(|t| t.name == c.kind);
                        for (key, label, value) in [
                            ("", "Date / time (UTC)", datetime.as_str()),
                            ("callsign", "Callsign / station", c.callsign.as_str()),
                            ("satellite", "Satellite / source", c.satellite.as_str()),
                            ("frequency", "Frequency (MHz)", c.frequency.as_str()),
                            ("mode", "Mode", c.mode.as_str()),
                            ("rst", "RST sent", c.rst_sent.as_str()),
                            ("rst", "RST received", c.rst_rcvd.as_str()),
                            ("location", "Station location / grid", c.location.as_str()),
                            ("recording", "Recording file / link", c.recording.as_str()),
                            ("notes", "Notes", c.notes.as_str()),
                        ] {
                            if value.trim().is_empty() && kind.is_some_and(|k| k.hides(key)) {
                                continue;
                            }
                            ui.label(RichText::new(label).color(Color32::from_gray(185)));
                            if value.trim().is_empty() {
                                ui.label("—");
                            } else {
                                ui.add(egui::Label::new(value).wrap());
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Edit").clicked() {
                        edit = true;
                    }
                    if ui.button("Close").clicked() {
                        self.viewing = None;
                    }
                });
            });
        if edit {
            self.draft = c;
            self.editing = true;
            self.viewing = None;
        } else if !open {
            self.viewing = None;
        }
    }
    fn types_window(&mut self, ctx: &egui::Context) {
        if !self.manage {
            return;
        }
        let mut open = true;
        egui::Window::new("Manage types")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_enabled_ui(self.pending.is_none() && !self.blocked, |ui| {
                    for t in self.log.types.clone() {
                        ui.horizontal(|ui| {
                            ui.colored_label(color(&t.color), &t.name);
                            ui.small(format!(
                                "{} entries",
                                self.log
                                    .contacts
                                    .iter()
                                    .filter(|c| c.kind == t.name)
                                    .count()
                            ));
                            if ui.button("Rename").clicked() {
                                self.rename = Some((t.name.clone(), t.name.clone()));
                            }
                            if ui.button("Colour").clicked() {
                                let mut next = self.log.clone();
                                let item =
                                    next.types.iter_mut().find(|x| x.name == t.name).unwrap();
                                let idx = model::COLORS
                                    .iter()
                                    .position(|x| *x == item.color)
                                    .unwrap_or(7);
                                item.color = model::COLORS[(idx + 1) % 8].into();
                                self.commit(next);
                            }
                            if ui
                                .add_enabled(self.log.types.len() > 1, egui::Button::new("Delete"))
                                .clicked()
                            {
                                self.pending = Some(Pending::DeleteType(t.name.clone()));
                            }
                        });
                        ui.collapsing(format!("Fields shown for {}", t.name), |ui| {
                            let mut next = self.log.clone();
                            let mut changed = false;
                            let item = next.types.iter_mut().find(|x| x.name == t.name).unwrap();
                            for (key, label) in model::FIELDS {
                                let mut shown = !item.hides(key);
                                if ui.checkbox(&mut shown, label).changed() {
                                    item.hidden.retain(|h| h != key);
                                    if !shown {
                                        item.hidden.push(key.into());
                                    }
                                    changed = true;
                                }
                            }
                            ui.small("Hidden fields still appear on records that already use them.");
                            if changed {
                                self.commit(next);
                            }
                        });
                    }
                    ui.separator();
                    if let Some((old, mut new)) = self.rename.clone() {
                        ui.label(format!("Rename {old}"));
                        ui.text_edit_singleline(&mut new);
                        self.rename = Some((old.clone(), new.clone()));
                        ui.horizontal(|ui| {
                            if ui.button("Save name").clicked() {
                                let mut next = self.log.clone();
                                match next.rename_type(&old, &new) {
                                    Ok(()) => {
                                        if self.commit(next) {
                                            let new = new.trim().to_string();
                                            if self.draft.kind == old {
                                                self.draft.kind = new.clone();
                                            }
                                            if self.type_filter == old {
                                                self.type_filter = new.clone();
                                            }
                                            if self
                                                .exact
                                                .as_ref()
                                                .is_some_and(|(f, v)| f == "Type" && v == &old)
                                            {
                                                self.exact = Some(("Type".into(), new));
                                            }
                                            self.rename = None;
                                        }
                                    }
                                    Err(e) => self.message = e,
                                }
                            }
                            if ui.button("Cancel rename").clicked() {
                                self.rename = None;
                            }
                        });
                        ui.separator();
                    }
                    ui.label("New type");
                    ui.text_edit_singleline(&mut self.new_type);
                    egui::ComboBox::from_id_salt("new-type-color")
                        .selected_text(model::COLORS[self.new_color])
                        .show_ui(ui, |ui| {
                            for (i, c) in model::COLORS.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.new_color,
                                    i,
                                    RichText::new(*c).color(color(c)),
                                );
                            }
                        });
                    if ui.button("Add type").clicked() {
                        let name = self.new_type.trim().to_string();
                        if name.is_empty()
                            || self
                                .log
                                .types
                                .iter()
                                .any(|t| model::key(&t.name) == model::key(&name))
                        {
                            self.message = "Use a non-empty, unique type name.".into();
                        } else {
                            let mut next = self.log.clone();
                            next.types
                                .push(Kind::new(name, model::COLORS[self.new_color].into()));
                            if self.commit(next) {
                                self.new_type.clear();
                            }
                        }
                    }
                });
            });
        self.manage = open;
    }
    fn confirmation(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending.clone() else {
            return;
        };
        egui::Window::new("Confirm change").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).show(ctx,|ui|{
            match pending {
                Pending::DeleteContact(id)=>{ui.label("Delete this contact? The previous log is backed up.");if ui.button("Delete contact").clicked(){let mut next=self.log.clone();next.contacts.retain(|c|c.id!=id);if self.commit(next){if self.draft.id==id{self.reset();}self.pending=None;}}},
                Pending::DeleteType(name)=>{let fallback=self.log.types.iter().find(|t|t.name!=name).map(|t|t.name.clone()).unwrap_or_default();ui.label(format!("Delete {name}? Its entries will move to {fallback}."));if ui.button("Delete type").clicked(){let mut next=self.log.clone();match next.delete_type(&name){Ok(fallback)=>if self.commit(next){if self.draft.kind==name{self.draft.kind=fallback;}self.type_filter.clear();self.exact=None;self.pending=None;},Err(e)=>self.message=e}}},
                Pending::Import(log)=>{ui.label(format!("Import {} contacts? Your existing log has {}.",log.contacts.len(),self.log.contacts.len()));ui.label("Merge keeps existing entries with matching IDs. Replace backs up the current log first.");ui.horizontal(|ui|{let mut next=None;if ui.button("Merge").clicked(){next=Some(self.log.merge(log.clone()));}if ui.button("Replace current log").clicked(){next=Some(log);}if let Some(next)=next{if self.commit(next){self.type_filter.clear();self.exact=None;self.search.clear();self.reset();self.pending=None;}}});},
            }
            if ui.button("Cancel").clicked(){self.pending=None;}
        });
    }
}

impl eframe::App for RadioLog {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_radio();
        if self.radio_rx.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Radio Log");
                ui.label(RichText::new("DESKTOP").small().color(color("teal")));
                ui.separator();
                ui.add_enabled_ui(!self.blocked && self.pending.is_none(), |ui| {
                    if ui.button("Import JSON").clicked() {
                        self.import();
                    }
                    if ui.button("Export JSON").clicked() {
                        self.export();
                    }
                    if ui.button("Manage types").clicked() {
                        self.manage = !self.manage;
                    }
                });
                if let Some(s) = &self.storage {
                    if ui
                        .button("Copy data folder")
                        .on_hover_text(s.root.display().to_string())
                        .clicked()
                    {
                        ctx.copy_text(s.root.display().to_string());
                    }
                }
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(RichText::new(&self.message).color(if self.blocked {
                color("rose")
            } else {
                Color32::from_gray(200)
            }));
        });
        egui::SidePanel::left("contact-form")
            .resizable(false)
            .exact_width(330.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_enabled_ui(!self.blocked && self.pending.is_none(), |ui| {
                        self.form(ui, ctx)
                    });
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_enabled_ui(!self.blocked && self.pending.is_none(), |ui| {
                self.entries(ui)
            });
        });
        self.view_window(ctx);
        self.types_window(ctx);
        self.confirmation(ctx);
    }
}

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|a| a == "--probe") {
        match rig::read_radio("127.0.0.1:4532".parse().unwrap()) {
            Ok(r) => println!(
                "{}",
                serde_json::json!({"frequencyMHz":r.mhz,"mode":r.mode})
            ),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let root = args
        .iter()
        .position(|a| a == "--data-dir")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            directories::ProjectDirs::from("", "SDR-Blog", "Radio Log")
                .expect("User data directory unavailable")
                .data_local_dir()
                .to_path_buf()
        });
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/radio-log.png"))
                    .expect("Invalid app icon"),
            )
            .with_inner_size([1320.0, 850.0])
            .with_min_inner_size([850.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Radio Log",
        options,
        Box::new(move |cc| Ok(Box::new(RadioLog::new(cc, root)))),
    )
}
