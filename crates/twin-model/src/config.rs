//! Point classes, mapping and limits: how raw telemetry becomes a condition.
//!
//! This is the customer-specific part of the twin. A site brings its own BMS point
//! names, asset ids and thresholds; the twin only ever reasons about *canonical classes*
//! (`liquid_return_temp`, `ups_mode`, …). See docs/mapping.md.
use crate::topo::Asset;
use crate::Tlp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Bad when the value rises (temperatures, load, vibration).
    High,
    /// Bad when the value falls (flow, level, state of charge).
    Low,
    /// Bad when the value leaves a band around `center` (frequency, voltage).
    Band,
}

/// Three thresholds per class. `watch` → Yellow (leading indicator), `degraded` → Orange,
/// `out` → Red. With `relative_to`, thresholds are fractions of an asset attribute
/// (e.g. 0.7 × `design_flow_lpm`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limit {
    pub dir: Direction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_to: Option<String>,
    /// For `band`: the nominal value (or the attribute holding it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<f64>,
}

/// One value of an enumerated point (status, mode, leak).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateDef {
    pub value: i64,
    pub text: String,
    pub tlp: Tlp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassDef {
    pub label: String,
    #[serde(default)]
    pub unit: String,
    /// The DSX BMS `ObjectType/PointType` the simulator publishes for this class.
    #[serde(default)]
    pub dsx: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<Limit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<StateDef>,
    /// Keep history and forecast with augurs.
    #[serde(default)]
    pub forecast: bool,
    /// The value is itself a time-to-empty in minutes (UPS runtime): it becomes the ETA.
    #[serde(default)]
    pub eta_minutes: bool,
}

/// Map a subject that is *not* described by DSX metadata onto an asset and class.
/// Tokens in braces capture one subject token: `twin.v1.health.{class}.{asset}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubjectRule {
    pub pattern: String,
    /// Optional fixed class when the pattern has no `{class}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TwinConfig {
    pub version: u64,
    /// Incoming point type → canonical class. Keys are `ObjectType/PointType` (DSX) or a
    /// bare `PointType`; the most specific match wins.
    #[serde(default)]
    pub point_types: BTreeMap<String, String>,
    /// Incoming object id (DSX `objectId`) → twin asset id, for sites whose BMS names differ.
    #[serde(default)]
    pub asset_aliases: BTreeMap<String, String>,
    #[serde(default)]
    pub subject_rules: Vec<SubjectRule>,
    pub classes: BTreeMap<String, ClassDef>,
}

/// The binding of one incoming subject to a twin point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointBinding {
    pub asset: String,
    pub class: String,
    pub point: String,
    #[serde(default)]
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub tlp: Tlp,
    pub text: String,
}

impl TwinConfig {
    /// Resolve a DSX object/point type pair to a canonical class.
    pub fn class_for(&self, object_type: &str, point_type: &str) -> Option<&str> {
        let full = format!("{object_type}/{point_type}");
        self.point_types
            .get(&full)
            .or_else(|| self.point_types.get(point_type))
            .map(String::as_str)
            // Default: the class whose own `dsx` type matches.
            .or_else(|| self.classes.iter().find(|(_, d)| d.dsx == full).map(|(k, _)| k.as_str()))
    }

    pub fn asset_id<'a>(&'a self, object_id: &'a str) -> &'a str {
        self.asset_aliases.get(object_id).map(String::as_str).unwrap_or(object_id)
    }

    /// Match a subject against the configured rules.
    pub fn match_rule(&self, subject: &str) -> Option<PointBinding> {
        let toks: Vec<&str> = subject.split('.').collect();
        for r in &self.subject_rules {
            let pat: Vec<&str> = r.pattern.split('.').collect();
            if pat.len() != toks.len() {
                continue;
            }
            let (mut asset, mut class) = (None, r.class.clone());
            let ok = pat.iter().zip(&toks).all(|(p, t)| match *p {
                "{asset}" => {
                    asset = Some(t.to_string());
                    true
                }
                "{class}" => {
                    class = Some(t.to_string());
                    true
                }
                "*" => true,
                lit => lit == *t,
            });
            if let (true, Some(a), Some(c)) = (ok, asset, class) {
                let a = self.asset_id(&a).to_string();
                let unit = self.classes.get(&c).map(|d| d.unit.clone()).unwrap_or_default();
                return Some(PointBinding { point: format!("{a}.{c}"), asset: a, class: c, unit });
            }
        }
        None
    }

    /// Evaluate a reading. `quality` follows DSX: 1 good, 0 bad.
    pub fn evaluate(&self, class: &str, value: f64, quality: i64, asset: Option<&Asset>) -> Condition {
        let Some(def) = self.classes.get(class) else {
            return Condition { tlp: Tlp::Green, text: String::new() };
        };
        if quality == 0 {
            return Condition { tlp: Tlp::Unknown, text: format!("{}: no data (bad quality)", def.label) };
        }
        if !def.states.is_empty() {
            let v = value.round() as i64;
            return match def.states.iter().find(|s| s.value == v) {
                Some(s) => Condition { tlp: s.tlp, text: format!("{}: {}", def.label, s.text) },
                None => Condition { tlp: Tlp::Unknown, text: format!("{}: unknown state {v}", def.label) },
            };
        }
        let Some(lim) = &def.limit else {
            return Condition { tlp: Tlp::Green, text: String::new() };
        };
        let scale = lim
            .relative_to
            .as_deref()
            .and_then(|k| asset.and_then(|a| a.attr(k)))
            .unwrap_or(1.0);
        let fmt = |x: f64| -> String {
            if lim.relative_to.is_some() {
                format!("{:.0}% of design", x * 100.0)
            } else {
                format!("{x:.1} {}", def.unit)
            }
        };
        let shown = format!("{:.1} {}", value, def.unit);
        let (tlp, edge) = match lim.dir {
            Direction::High => {
                let x = value / scale;
                pick(&[(lim.out, Tlp::Red), (lim.degraded, Tlp::Orange), (lim.watch, Tlp::Yellow)], |t| x >= t)
            }
            Direction::Low => {
                let x = value / scale;
                pick(&[(lim.out, Tlp::Red), (lim.degraded, Tlp::Orange), (lim.watch, Tlp::Yellow)], |t| x <= t)
            }
            Direction::Band => {
                let c = lim.center.unwrap_or(0.0) * scale;
                let dev = (value - c).abs() / scale;
                pick(&[(lim.out, Tlp::Red), (lim.degraded, Tlp::Orange), (lim.watch, Tlp::Yellow)], |t| dev >= t)
            }
        };
        let text = match (tlp, edge) {
            (Tlp::Green, _) | (_, None) => format!("{} {shown}", def.label),
            (t, Some(e)) => {
                let word = match lim.dir {
                    Direction::High => "above",
                    Direction::Low => "below",
                    Direction::Band => "off nominal by",
                };
                let lvl = match t {
                    Tlp::Red => "out",
                    Tlp::Orange => "degraded",
                    _ => "watch",
                };
                format!("{} {shown} ({lvl}: {word} {})", def.label, fmt(e))
            }
        };
        Condition { tlp, text }
    }

    /// The value at which a forecast for this class turns Orange (degraded), in raw units.
    pub fn degraded_threshold(&self, class: &str, asset: Option<&Asset>) -> Option<(Direction, f64)> {
        let lim = self.classes.get(class)?.limit.as_ref()?;
        let scale = lim
            .relative_to
            .as_deref()
            .and_then(|k| asset.and_then(|a| a.attr(k)))
            .unwrap_or(1.0);
        let t = lim.degraded?;
        match lim.dir {
            Direction::Band => None,
            d => Some((d, t * scale)),
        }
    }
}

fn pick(levels: &[(Option<f64>, Tlp)], hit: impl Fn(f64) -> bool) -> (Tlp, Option<f64>) {
    for (t, tlp) in levels {
        if let Some(t) = t {
            if hit(*t) {
                return (*tlp, Some(*t));
            }
        }
    }
    (Tlp::Green, None)
}
