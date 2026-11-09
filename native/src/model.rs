use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

pub const COLORS: [&str; 8] = [
    "emerald", "violet", "amber", "sky", "rose", "orange", "teal", "slate",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Kind {
    pub name: String,
    pub color: String,
    /// Optional fields (keys from `FIELDS`) this type does not show in the form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
}

/// Optional form fields a type may hide: (key, label).
pub const FIELDS: [(&str, &str); 8] = [
    ("callsign", "Callsign / station"),
    ("satellite", "Satellite / source"),
    ("frequency", "Frequency (MHz)"),
    ("mode", "Mode"),
    ("rst", "RST sent / received"),
    ("location", "Station location / grid"),
    ("recording", "Recording file / link"),
    ("notes", "Notes"),
];

impl Kind {
    pub fn new(name: String, color: String) -> Self {
        Self {
            name,
            color,
            hidden: vec![],
        }
    }
    pub fn hides(&self, field: &str) -> bool {
        self.hidden.iter().any(|h| h == field)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Contact {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub datetime: String,
    pub callsign: String,
    pub satellite: String,
    pub frequency: String,
    pub mode: String,
    pub rst_sent: String,
    pub rst_rcvd: String,
    pub location: String,
    pub recording: String,
    pub notes: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Log {
    pub version: u32,
    pub types: Vec<Kind>,
    pub contacts: Vec<Contact>,
}

impl Default for Log {
    fn default() -> Self {
        Self {
            version: 2,
            types: [
                ("Ham", "emerald", &["satellite", "recording"][..]),
                ("Satellite", "violet", &["rst", "location"][..]),
                ("SDR Recording", "amber", &["callsign", "rst"][..]),
                ("Other", "slate", &[][..]),
            ]
            .into_iter()
            .map(|(n, c, hidden)| Kind {
                hidden: hidden.iter().map(|h| h.to_string()).collect(),
                ..Kind::new(n.into(), c.into())
            })
            .collect(),
            contacts: vec![],
        }
    }
}

pub fn now() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M").to_string()
}
pub fn key(s: &str) -> String {
    s.trim().to_uppercase()
}
pub fn canonical_date(s: &str) -> Result<String, String> {
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Ok(d.with_timezone(&Utc).format("%Y-%m-%dT%H:%M").to_string());
    }
    for fmt in ["%Y-%m-%dT%H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(d) = NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(d.format("%Y-%m-%dT%H:%M").to_string());
        }
    }
    Err("Enter UTC date/time as YYYY-MM-DD HH:MM.".into())
}

impl Contact {
    pub fn fresh(kind: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            datetime: now(),
            ..Self::default()
        }
    }
    pub fn validate(&mut self) -> Result<(), String> {
        self.datetime = canonical_date(&self.datetime)?;
        self.callsign = key(&self.callsign);
        if !self.frequency.trim().is_empty() {
            let f = self
                .frequency
                .parse::<f64>()
                .map_err(|_| "Frequency must be a positive number in MHz.")?;
            if !f.is_finite() || f <= 0.0 {
                return Err("Frequency must be a positive number in MHz.".into());
            }
        }
        Ok(())
    }
    pub fn field(&self, f: &str) -> &str {
        match f {
            "Type" => &self.kind,
            "Callsign" => &self.callsign,
            "Source" => &self.satellite,
            "Frequency" => &self.frequency,
            "Mode" => &self.mode,
            "Location" => &self.location,
            _ => "",
        }
    }
    pub fn matches(&self, f: &str, value: &str) -> bool {
        if f == "Frequency" {
            if let (Ok(a), Ok(b)) = (self.frequency.parse::<f64>(), value.parse::<f64>()) {
                return a == b;
            }
        }
        key(self.field(f)) == key(value)
    }
    pub fn searchable(&self) -> String {
        [
            &self.kind,
            &self.datetime,
            &self.callsign,
            &self.satellite,
            &self.frequency,
            &self.mode,
            &self.rst_sent,
            &self.rst_rcvd,
            &self.location,
            &self.recording,
            &self.notes,
        ]
        .into_iter()
        .map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
    }
}

impl Log {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut v: Value =
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid JSON: {e}"))?;
        if v.is_array() {
            v = serde_json::json!({"contacts":v});
        }
        let obj = v
            .as_object_mut()
            .ok_or("Expected a log object or contact array.")?;
        if let Some(version) = obj.get("version") {
            if version != 1 && version != 2 {
                return Err("Unsupported log version.".into());
            }
        }
        let raw = obj
            .get("contacts")
            .and_then(Value::as_array)
            .ok_or("No contacts array found.")?;
        let mut log = Self::default();
        if let Some(t) = obj.get("types") {
            log.types = serde_json::from_value(t.clone()).map_err(|_| "Invalid types list.")?;
            if log.types.is_empty() {
                log.types = Self::default().types;
            }
        }
        let mut type_names = HashSet::new();
        for t in &mut log.types {
            t.name = t.name.trim().into();
            if t.name.is_empty() || !type_names.insert(key(&t.name)) {
                return Err("Empty or duplicate type names.".into());
            }
            if !COLORS.contains(&t.color.as_str()) {
                t.color = "slate".into();
            }
        }
        let mut ids = HashSet::new();
        for item in raw {
            let mut item = item.as_object().ok_or("Invalid contact record.")?.clone();
            for f in [
                "id",
                "type",
                "datetime",
                "callsign",
                "satellite",
                "frequency",
                "mode",
                "rstSent",
                "rstRcvd",
                "location",
                "recording",
                "notes",
            ] {
                let s = match item.get(f) {
                    None | Some(Value::Null) => String::new(),
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Number(n)) => n.to_string(),
                    _ => return Err(format!("Invalid contact field: {f}")),
                };
                item.insert(f.into(), Value::String(s));
            }
            let mut c: Contact =
                serde_json::from_value(Value::Object(item)).map_err(|e| e.to_string())?;
            if c.id.is_empty() {
                c.id = uuid::Uuid::new_v4().to_string();
            }
            if !ids.insert(c.id.clone()) {
                return Err("Duplicate contact IDs in the file.".into());
            }
            if c.datetime.is_empty() {
                c.datetime = now();
            } else {
                c.datetime = canonical_date(&c.datetime)?;
            }
            if c.kind.trim().is_empty() {
                c.kind = "Other".into();
            }
            if let Some(t) = log.types.iter().find(|t| key(&t.name) == key(&c.kind)) {
                c.kind = t.name.clone();
            } else {
                log.types.push(Kind::new(c.kind.clone(), "slate".into()));
            }
            log.contacts.push(c);
        }
        Ok(log)
    }
    pub fn merge(&self, mut incoming: Self) -> Self {
        let mut result = self.clone();
        for t in incoming.types {
            if let Some(existing) = result.types.iter().find(|x| key(&x.name) == key(&t.name)) {
                for c in &mut incoming.contacts {
                    if c.kind == t.name {
                        c.kind = existing.name.clone();
                    }
                }
            } else {
                result.types.push(t);
            }
        }
        let ids: HashSet<_> = result.contacts.iter().map(|c| c.id.clone()).collect();
        result.contacts.extend(
            incoming
                .contacts
                .into_iter()
                .filter(|c| !ids.contains(&c.id)),
        );
        result
    }
    pub fn rename_type(&mut self, old: &str, new: &str) -> Result<(), String> {
        let new = new.trim();
        if new.is_empty() {
            return Err("Type name cannot be blank.".into());
        }
        if self
            .types
            .iter()
            .any(|t| t.name != old && key(&t.name) == key(new))
        {
            return Err("That type already exists.".into());
        }
        self.types
            .iter_mut()
            .find(|t| t.name == old)
            .ok_or("Type not found.")?
            .name = new.into();
        for c in &mut self.contacts {
            if c.kind == old {
                c.kind = new.into();
            }
        }
        Ok(())
    }
    pub fn delete_type(&mut self, name: &str) -> Result<String, String> {
        if self.types.len() <= 1 {
            return Err("Keep at least one type.".into());
        }
        let fallback = self
            .types
            .iter()
            .find(|t| t.name != name)
            .ok_or("Type not found.")?
            .name
            .clone();
        self.types.retain(|t| t.name != name);
        for c in &mut self.contacts {
            if c.kind == name {
                c.kind = fallback.clone();
            }
        }
        Ok(fallback)
    }
}

pub fn infer_type(mhz: f64, mode: &str) -> &'static str {
    if ["APT", "LRPT", "HRPT"].contains(&key(mode).as_str()) || (137.0..138.0).contains(&mhz) {
        return "Satellite";
    }
    let bands = [
        (0.1357, 0.1378),
        (0.472, 0.479),
        (1.81, 2.0),
        (3.5, 3.8),
        (5.2585, 5.264),
        (5.276, 5.284),
        (5.288, 5.292),
        (5.298, 5.307),
        (5.313, 5.323),
        (5.333, 5.338),
        (5.354, 5.358),
        (5.362, 5.3745),
        (5.378, 5.382),
        (5.395, 5.4015),
        (5.4035, 5.4065),
        (7.0, 7.2),
        (10.1, 10.15),
        (14.0, 14.35),
        (18.068, 18.168),
        (21.0, 21.45),
        (24.89, 24.99),
        (28.0, 29.7),
        (50.0, 52.0),
        (70.0, 70.5),
        (144.0, 146.0),
        (430.0, 440.0),
        (1240.0, 1325.0),
    ];
    if bands.iter().any(|(a, b)| mhz >= *a && mhz <= *b) {
        "Ham"
    } else {
        "Other"
    }
}
