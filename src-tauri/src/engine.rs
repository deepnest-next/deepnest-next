//! The nesting pipeline behind the app's commands, linked in-process — the same steps as the
//! API server and the `deepnest` CLI:
//! DXF/SVG text -> (nest-adapters import) -> parts in mm -> babushka engine -> `Layout`
//! -> (nest-adapters export) -> placed geometry for the viewport, or a DXF/SVG cut file.
//!
//! Nothing here depends on Tauri, so it is unit-tested directly.

use std::collections::{BTreeMap, BTreeSet};

use deepnest_dxf2svg_processor::conversions;
use nest_adapters::export::{
    PlacedPart, apply_kerf, apply_leads, place_layout, rect_frame, to_drawing, to_svg_framed,
};
use nest_adapters::import::{
    NestParams, build_nest_request, flatten_drawing, insunits_scale_to_mm, normalize_svg_drawing,
    rectangular_bins,
};
use nesting_babushka::BabushkaBackend;
use nesting_core::{
    ContainerKind, NestConfig, NestRequest, NestingBackend, Part, PartId, Point, Polygon,
    RotationSet, Strategy, UnplacedReason,
};
use serde::{Deserialize, Serialize};

/// Same threshold the engine (and the API server) uses to reject degenerate parts.
const DEGENERATE_AREA_EPS: f64 = 1e-9;

pub type CardId = u32;

/// One imported file: a "part card" in the UI. A file may hold several separate shapes; the
/// card's quantity/rotation settings apply to each of them.
struct Card {
    name: String,
    shapes: Vec<Part>,
}

/// What the UI shows for an imported card.
#[derive(Serialize, Debug)]
pub struct CardInfo {
    pub id: CardId,
    pub name: String,
    /// Each shape (outer + holes) in mm.
    pub shapes: Vec<Polygon>,
    pub width: f64,
    pub height: f64,
    pub warnings: Vec<String>,
}

#[derive(Deserialize)]
pub struct JobItem {
    pub id: CardId,
    pub quantity: u32,
    pub rotations: u32,
    #[serde(default)]
    pub mirror: bool,
}

#[derive(Deserialize)]
pub struct NestJob {
    pub items: Vec<JobItem>,
    pub sheet_width: f64,
    pub sheet_height: f64,
    /// `None` = as many sheets as needed.
    pub sheets: Option<u32>,
    pub spacing: f64,
    pub margin: f64,
    /// `fast` | `balanced` | `high_quality`.
    pub strategy: String,
    pub time_ms: u64,
    #[serde(default)]
    pub seed: u64,
}

#[derive(Serialize, Debug)]
pub struct PlacedShape {
    pub card: CardId,
    pub polygon: Polygon,
}

#[derive(Serialize, Debug)]
pub struct SheetOut {
    pub id: u64,
    pub width: f64,
    pub height: f64,
    pub parts: Vec<PlacedShape>,
}

#[derive(Serialize, Debug)]
pub struct Unplaced {
    pub card: CardId,
    pub name: String,
    pub reason: &'static str,
    pub count: u32,
}

#[derive(Serialize, Debug)]
pub struct NestOutcome {
    pub sheets: Vec<SheetOut>,
    pub placed: u32,
    pub required_placed: u32,
    pub required_total: u32,
    pub utilization: f64,
    pub runtime_ms: u64,
    pub unplaced: Vec<Unplaced>,
}

/// The last nest, kept for export.
struct LastNest {
    placed: Vec<PlacedPart>,
    sheet_ids: Vec<u64>,
    sheet: (f64, f64),
}

#[derive(Default)]
pub struct Library {
    next_id: CardId,
    cards: BTreeMap<CardId, Card>,
    last: Option<LastNest>,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Svg,
    Dxf,
}

impl Library {
    /// Import one DXF or SVG file (by extension, else sniffed) as a new card, scaled to mm from
    /// the DXF `$INSUNITS` header or the SVG's physical root size; unitless files are used as-is
    /// with a warning.
    pub fn import(&mut self, name: &str, content: &str) -> Result<CardInfo, String> {
        let svg = is_svg(name, content);
        let drawing = if svg {
            let dxf = conversions::svg_to_dxf(content).map_err(|e| format!("invalid SVG: {e}"))?;
            let mut d = conversions::parse_dxf_bytes(&dxf)
                .map_err(|e| format!("SVG conversion failed: {e}"))?;
            normalize_svg_drawing(content, &mut d);
            d
        } else {
            conversions::parse_dxf(content).map_err(|e| format!("invalid DXF: {e}"))?
        };

        let mut warnings = Vec::new();
        let unit_scale = insunits_scale_to_mm(&drawing).unwrap_or_else(|| {
            warnings.push(if svg {
                "SVG has no physical width/height units; drawing units used as mm".to_string()
            } else {
                "DXF $INSUNITS is unitless/unknown; drawing units used as mm".to_string()
            });
            1.0
        });
        let params = NestParams {
            unit_scale,
            ..NestParams::default()
        };
        // `build_nest_request`, not `group_contours_into_parts`: only it applies `unit_scale`.
        let (req, report) = build_nest_request(&flatten_drawing(&drawing), &params);
        warnings.extend(report.warnings.iter().map(|w| w.to_string()));
        let total = req.parts.len();
        let shapes: Vec<Part> = req.parts.into_iter().filter(part_is_nestable).collect();
        if shapes.len() < total {
            warnings.push(format!(
                "{} degenerate shape(s) skipped",
                total - shapes.len()
            ));
        }
        if shapes.is_empty() {
            return Err("no closed contours to nest".to_string());
        }

        let id = self.next_id;
        self.next_id += 1;
        let card = Card {
            name: name.to_string(),
            shapes,
        };
        let info = card.info(id, warnings);
        self.cards.insert(id, card);
        Ok(info)
    }

    pub fn remove(&mut self, id: CardId) {
        self.cards.remove(&id);
    }

    /// The engine request for `job`, plus the card each engine part id came from.
    fn request(&self, job: &NestJob) -> Result<(NestRequest, BTreeMap<PartId, CardId>), String> {
        if !(job.sheet_width > 0.0 && job.sheet_height > 0.0) {
            return Err("sheet width and height must be > 0".to_string());
        }
        let mut parts = Vec::new();
        let mut owner = BTreeMap::new();
        for item in job.items.iter().filter(|i| i.quantity > 0) {
            let card = self
                .cards
                .get(&item.id)
                .ok_or_else(|| format!("unknown part {}", item.id))?;
            for shape in &card.shapes {
                let id = parts.len() as PartId;
                owner.insert(id, item.id);
                parts.push(Part {
                    id,
                    quantity: item.quantity,
                    rotation: rotation_set(item.rotations),
                    allow_mirror: item.mirror,
                    ..shape.clone()
                });
            }
        }
        if parts.is_empty() {
            return Err("nothing to nest: add parts with a quantity above 0".to_string());
        }
        let request = NestRequest {
            parts,
            bins: rectangular_bins(job.sheet_width, job.sheet_height, job.sheets),
            config: NestConfig {
                strategy: match job.strategy.as_str() {
                    "fast" => Strategy::Fast,
                    "high_quality" => Strategy::HighQuality,
                    _ => Strategy::Balanced,
                },
                seed: job.seed,
                part_spacing: job.spacing.max(0.0),
                bin_margin: job.margin.max(0.0),
                time_budget_ms: Some(job.time_ms.max(100)),
            },
        };
        Ok((request, owner))
    }

    /// Run the engine. Blocking: call it off the UI thread.
    pub fn nest(&mut self, job: &NestJob) -> Result<NestOutcome, String> {
        let (request, owner) = self.request(job)?;
        let layout = BabushkaBackend
            .nest(&request)
            .map_err(|e| format!("nesting failed: {e}"))?;
        let placed = place_layout(&request.parts, &layout)?;

        // Sheets in the order the engine opened them; fall back to placement order.
        let mut sheet_ids: Vec<u64> = layout.bins.iter().map(|b| b.container_id).collect();
        if sheet_ids.is_empty() {
            let seen: BTreeSet<u64> = layout.placements.iter().map(|p| p.container_id).collect();
            sheet_ids = seen.into_iter().collect();
        }
        let sheets = sheet_ids
            .iter()
            .map(|&id| SheetOut {
                id,
                width: job.sheet_width,
                height: job.sheet_height,
                parts: placed
                    .iter()
                    .filter(|p| p.container_id == id)
                    .map(|p| PlacedShape {
                        card: owner[&p.part_id],
                        polygon: p.polygon.clone(),
                    })
                    .collect(),
            })
            .collect();

        let mut agg: BTreeMap<(CardId, &'static str), u32> = BTreeMap::new();
        for (i, inst) in layout.unplaced.iter().enumerate() {
            let reason = match layout.unplaced_reasons.get(i) {
                Some(UnplacedReason::TooLargeForAnyBin) => "too large for the sheet",
                Some(UnplacedReason::NoRemainingCapacity) => "no room left",
                None => "not placed",
            };
            *agg.entry((owner[&inst.part_id], reason)).or_insert(0) += 1;
        }
        let unplaced = agg
            .into_iter()
            .map(|((card, reason), count)| Unplaced {
                card,
                name: self.cards[&card].name.clone(),
                reason,
                count,
            })
            .collect();

        let m = &layout.metrics;
        let outcome = NestOutcome {
            sheets,
            placed: m.placed,
            required_placed: m.required_placed,
            required_total: m.required_total,
            utilization: m.utilization,
            runtime_ms: m.runtime_ms,
            unplaced,
        };
        self.last = Some(LastNest {
            placed,
            sheet_ids,
            sheet: (job.sheet_width, job.sheet_height),
        });
        Ok(outcome)
    }

    /// The last nest as a cut file. `kerf` offsets each cut outward by half its width; `lead_in`
    /// adds pierce lead-ins of that length (both mm, 0 = off).
    pub fn export(&self, format: Format, kerf: f64, lead_in: f64) -> Result<Vec<u8>, String> {
        let last = self.last.as_ref().ok_or("nothing nested yet")?;
        let mut placed = last.placed.clone();
        if kerf > 0.0 {
            apply_kerf(&mut placed, kerf);
        }
        if lead_in > 0.0 {
            apply_leads(&mut placed, lead_in);
        }
        Ok(match format {
            Format::Svg => {
                let (w, h) = last.sheet;
                let frames: Vec<_> = last
                    .sheet_ids
                    .iter()
                    .map(|&id| rect_frame(id, w, h, ContainerKind::Sheet))
                    .collect();
                to_svg_framed(&placed, &frames).into_bytes()
            }
            Format::Dxf => {
                let mut buf = Vec::new();
                to_drawing(&placed)
                    .save(&mut buf)
                    .map_err(|e| format!("encoding DXF: {e}"))?;
                buf
            }
        })
    }
}

impl Card {
    fn info(&self, id: CardId, warnings: Vec<String>) -> CardInfo {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in self.shapes.iter().flat_map(|s| &s.polygon.outer) {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        CardInfo {
            id,
            name: self.name.clone(),
            shapes: self.shapes.iter().map(|s| s.polygon.clone()).collect(),
            width: x1 - x0,
            height: y1 - y0,
            warnings,
        }
    }
}

fn is_svg(name: &str, content: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".svg") {
        return true;
    }
    if lower.ends_with(".dxf") {
        return false;
    }
    let head: String = content.chars().take(1024).collect();
    head.contains("<svg")
}

fn rotation_set(n: u32) -> RotationSet {
    if n <= 1 {
        RotationSet::None
    } else {
        RotationSet::Discrete {
            angles_deg: (0..n).map(|i| 360.0 * i as f64 / n as f64).collect(),
        }
    }
}

/// Mirrors the engine's own reject condition (and the API server's `part_is_nestable`).
fn part_is_nestable(p: &Part) -> bool {
    let finite = |ring: &[Point]| ring.iter().all(|q| q.x.is_finite() && q.y.is_finite());
    finite(&p.polygon.outer)
        && p.polygon.holes.iter().all(|h| finite(h))
        && p.polygon.outer.len() >= 3
        && nesting_core::polygon_area(&p.polygon) > DEGENERATE_AREA_EPS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inkscape-style: 100 mm page; a 40x20 mm plate with a hole, plus a separate 10x10 square.
    const MM_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100">
      <rect x="0" y="0" width="40" height="20" fill="none" stroke="#000"/>
      <circle cx="20" cy="10" r="4" fill="none" stroke="#000"/>
      <rect x="60" y="0" width="10" height="10" fill="none" stroke="#000"/>
    </svg>"##;

    fn job(items: Vec<JobItem>, sheets: Option<u32>) -> NestJob {
        NestJob {
            items,
            sheet_width: 100.0,
            sheet_height: 50.0,
            sheets,
            spacing: 1.0,
            margin: 0.0,
            strategy: "fast".to_string(),
            time_ms: 500,
            seed: 1,
        }
    }

    fn item(id: CardId, quantity: u32) -> JobItem {
        JobItem {
            id,
            quantity,
            rotations: 4,
            mirror: false,
        }
    }

    #[test]
    fn import_svg_in_mm_with_holes() {
        let mut lib = Library::default();
        let card = lib.import("plate.svg", MM_SVG).unwrap();
        assert_eq!(card.shapes.len(), 2, "plate + square");
        assert_eq!(card.shapes[0].holes.len(), 1, "circle is a hole");
        assert!((card.width - 70.0).abs() < 1e-3, "width {}", card.width);
        assert!((card.height - 20.0).abs() < 1e-3, "height {}", card.height);
        assert!(card.warnings.is_empty(), "{:?}", card.warnings);
    }

    #[test]
    fn unitless_svg_warns() {
        let mut lib = Library::default();
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="5" height="5"/></svg>"#;
        let card = lib.import("x.svg", svg).unwrap();
        assert!(
            card.warnings[0].contains("no physical"),
            "{:?}",
            card.warnings
        );
    }

    #[test]
    fn invalid_or_empty_input_is_an_error() {
        let mut lib = Library::default();
        assert!(lib.import("a.dxf", "not a dxf").is_err());
        let open = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1in" height="1in"><path d="M0 0 L10 10" stroke="#000"/></svg>"##;
        assert!(
            lib.import("open.svg", open)
                .unwrap_err()
                .contains("no closed")
        );
    }

    #[test]
    fn nest_places_every_copy_and_exports() {
        let mut lib = Library::default();
        let id = lib.import("plate.svg", MM_SVG).unwrap().id;
        let out = lib.nest(&job(vec![item(id, 3)], None)).unwrap();
        // 3 copies of 2 shapes.
        assert_eq!(out.required_total, 6);
        assert_eq!(out.required_placed, 6);
        let shapes: usize = out.sheets.iter().map(|s| s.parts.len()).sum();
        assert_eq!(shapes, 6);
        assert!(
            out.sheets
                .iter()
                .flat_map(|s| &s.parts)
                .all(|p| p.card == id)
        );

        let svg = String::from_utf8(lib.export(Format::Svg, 0.2, 2.0).unwrap()).unwrap();
        assert!(svg.starts_with("<svg") || svg.contains("<svg"));
        let dxf = String::from_utf8(lib.export(Format::Dxf, 0.0, 0.0).unwrap()).unwrap();
        assert!(dxf.contains("ENTITIES"));
    }

    #[test]
    fn shortfall_is_reported_per_card() {
        let mut lib = Library::default();
        let id = lib.import("plate.svg", MM_SVG).unwrap().id;
        let out = lib.nest(&job(vec![item(id, 40)], Some(1))).unwrap();
        assert!(out.required_placed < out.required_total);
        assert_eq!(out.unplaced[0].name, "plate.svg");
        assert_eq!(out.unplaced[0].card, id);
    }

    #[test]
    fn job_validation() {
        let mut lib = Library::default();
        assert!(
            lib.nest(&job(vec![], None))
                .unwrap_err()
                .contains("nothing to nest")
        );
        assert!(
            lib.nest(&job(vec![item(7, 1)], None))
                .unwrap_err()
                .contains("unknown part")
        );
        assert!(lib.export(Format::Svg, 0.0, 0.0).is_err(), "no nest yet");
        let id = lib.import("plate.svg", MM_SVG).unwrap().id;
        lib.remove(id);
        assert!(lib.nest(&job(vec![item(id, 1)], None)).is_err());
    }
}
