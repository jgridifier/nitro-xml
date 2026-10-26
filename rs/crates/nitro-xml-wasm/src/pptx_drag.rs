// SPDX-License-Identifier: MIT
//! Pointer-driven move, resize and rotate of the selected pptx shapes, plus arrow-key nudge.
//!
//! The drag lives in the engine so it stays ordered with the pointer events that start and end
//! it. While it runs only the selection chrome follows the pointer (`PptxState::live`); release
//! queues one `patchShapeGeometry` per moved shape, which `process_frame` applies through the
//! normal edit path (undoable, saved with the deck).

use nitro_xml_pptx::render::{handle_centers, rotate_about, SelectionBox, RESIZE_HANDLES};
use serde_json::json;

use crate::events::{MOD_CTRL, MOD_META, MOD_SHIFT};
use crate::pptx::PptxState;

/// Screen pixels of pointer travel before a press on a shape becomes a drag.
const DRAG_START_PX: f64 = 3.0;
/// Screen pixels around a handle centre that grab it.
const HANDLE_HIT_PX: f64 = 8.0;
/// Smallest width / height a resize can reach, in slide pixels.
const MIN_SIZE: f64 = 8.0;
/// Arrow nudge in slide pixels; Ctrl nudges by one.
const NUDGE: f64 = 8.0;
const SNAP_DEG: f64 = 15.0;

#[derive(Clone, Copy)]
enum Kind {
    Move,
    Resize(&'static str),
    Rotate,
}

pub(crate) struct Drag {
    kind: Kind,
    /// Press point in screen pixels.
    start: (f64, f64),
    orig: Vec<SelectionBox>,
    active: bool,
    /// Pointer angle about the shape centre at press, for rotate.
    angle0: f64,
}

fn deg_at(b: &SelectionBox, px: f64, py: f64) -> f64 {
    (py - (b.y + b.height / 2.0))
        .atan2(px - (b.x + b.width / 2.0))
        .to_degrees()
}

impl PptxState {
    fn box_of(&mut self, id: &str) -> Option<SelectionBox> {
        let active = self.active;
        let slide = self.pres.slide(active).ok()?;
        let el = slide.elements.iter().find(|e| e.id == id)?;
        Some(SelectionBox {
            id: el.id.clone(),
            x: el.x,
            y: el.y,
            width: el.width,
            height: el.height,
            rotation: el.rotation.unwrap_or(0.0),
        })
    }

    pub(crate) fn take_pending_edits(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_edits)
    }

    pub(crate) fn cancel_drag(&mut self) -> bool {
        self.live.clear();
        self.drag.take().is_some()
    }

    pub(crate) fn pointer_down(&mut self, x: f32, y: f32, mods: u8) -> bool {
        self.cancel_drag();
        let scale = self.effective_scale();
        if scale <= f64::EPSILON {
            return false;
        }
        let (sx, sy) = (f64::from(x) / scale, f64::from(y) / scale);
        if self.interactive && self.selected_ids.len() == 1 {
            let id = self.selected_ids[0].clone();
            if let Some(b) = self.box_of(&id) {
                for (name, cx, cy) in handle_centers(&b, scale) {
                    if (sx - cx).hypot(sy - cy) * scale <= HANDLE_HIT_PX {
                        let kind = if name == "rot" {
                            Kind::Rotate
                        } else {
                            Kind::Resize(name)
                        };
                        self.drag = Some(Drag {
                            kind,
                            start: (f64::from(x), f64::from(y)),
                            angle0: deg_at(&b, sx, sy),
                            orig: vec![b],
                            active: false,
                        });
                        return false;
                    }
                }
            }
        }
        let toggle = mods & (MOD_SHIFT | MOD_CTRL | MOD_META) != 0;
        match self.hit_id(x, y) {
            Some(id) => {
                let held = self.selected_ids.contains(&id);
                if toggle {
                    if held {
                        self.selected_ids.retain(|s| s != &id);
                    } else {
                        self.selected_ids.push(id.clone());
                    }
                } else if !held {
                    self.selected_ids = vec![id.clone()];
                }
                if self.interactive && self.selected_ids.contains(&id) {
                    let ids = self.selected_ids.clone();
                    let orig: Vec<SelectionBox> =
                        ids.iter().filter_map(|i| self.box_of(i)).collect();
                    self.drag = Some(Drag {
                        kind: Kind::Move,
                        start: (f64::from(x), f64::from(y)),
                        orig,
                        active: false,
                        angle0: 0.0,
                    });
                }
            }
            None if !toggle => self.selected_ids.clear(),
            None => {}
        }
        true
    }

    pub(crate) fn pointer_move(&mut self, x: f32, y: f32, buttons: u16, mods: u8) -> bool {
        let Some(mut d) = self.drag.take() else {
            return false;
        };
        if buttons & 1 == 0 {
            // The release happened outside the surface.
            self.live.clear();
            return false;
        }
        let scale = self.effective_scale();
        let (mx, my) = (f64::from(x), f64::from(y));
        let (dx, dy) = (mx - d.start.0, my - d.start.1);
        if !d.active {
            if dx.hypot(dy) < DRAG_START_PX {
                self.drag = Some(d);
                return false;
            }
            d.active = true;
        }
        let shift = mods & MOD_SHIFT != 0;
        let (px, py) = (mx / scale, my / scale);
        self.live = match d.kind {
            Kind::Move => {
                let (mut ddx, mut ddy) = (dx / scale, dy / scale);
                if shift {
                    if ddx.abs() >= ddy.abs() {
                        ddy = 0.0;
                    } else {
                        ddx = 0.0;
                    }
                }
                d.orig
                    .iter()
                    .map(|o| SelectionBox {
                        x: o.x + ddx,
                        y: o.y + ddy,
                        ..o.clone()
                    })
                    .collect()
            }
            Kind::Rotate => {
                let o = &d.orig[0];
                let mut rot = o.rotation + deg_at(o, px, py) - d.angle0;
                if shift {
                    rot = (rot / SNAP_DEG).round() * SNAP_DEG;
                }
                vec![SelectionBox {
                    rotation: rot.rem_euclid(360.0),
                    ..o.clone()
                }]
            }
            Kind::Resize(name) => vec![resized(&d.orig[0], name, px, py, shift)],
        };
        self.drag = Some(d);
        true
    }

    pub(crate) fn pointer_up(&mut self) -> bool {
        let Some(d) = self.drag.take() else {
            return false;
        };
        let live = std::mem::take(&mut self.live);
        if !d.active {
            return false;
        }
        for (o, l) in d.orig.iter().zip(live.iter()) {
            if let Some(spec) = geometry_spec(self.active, o, l, d.kind) {
                self.pending_edits.push(spec);
            }
        }
        true
    }

    /// Arrow keys nudge the selected shapes. `None` when nothing is selected.
    pub(crate) fn nudge(&mut self, key: &str, ctrl: bool) -> Option<Vec<String>> {
        let step = if ctrl { 1.0 } else { NUDGE };
        let (dx, dy) = match key {
            "ArrowLeft" => (-step, 0.0),
            "ArrowRight" => (step, 0.0),
            "ArrowUp" => (0.0, -step),
            "ArrowDown" => (0.0, step),
            _ => return None,
        };
        let ids = self.selected_ids.clone();
        let slide = self.active;
        let specs: Vec<String> = ids
            .iter()
            .filter_map(|id| self.box_of(id))
            .filter_map(|o| {
                let l = SelectionBox {
                    x: o.x + dx,
                    y: o.y + dy,
                    ..o.clone()
                };
                geometry_spec(slide, &o, &l, Kind::Move)
            })
            .collect();
        (!specs.is_empty()).then_some(specs)
    }
}

/// New box for dragging resize handle `name` to slide point `(px, py)`. The opposite edge
/// (or corner) stays where it is on the slide, also when the shape is rotated.
fn resized(o: &SelectionBox, name: &str, px: f64, py: f64, keep_aspect: bool) -> SelectionBox {
    let Some(&(_, fx, fy, _)) = RESIZE_HANDLES.iter().find(|h| h.0 == name) else {
        return o.clone();
    };
    let (sx, sy) = (fx * 2.0 - 1.0, fy * 2.0 - 1.0);
    let (cx, cy) = (o.x + o.width / 2.0, o.y + o.height / 2.0);
    let (ax, ay) = rotate_about(
        o.x + (0.5 - sx * 0.5) * o.width,
        o.y + (0.5 - sy * 0.5) * o.height,
        cx,
        cy,
        o.rotation,
    );
    let (lx, ly) = rotate_about(px, py, ax, ay, -o.rotation);
    let (lx, ly) = (lx - ax, ly - ay);
    let mut w = if sx != 0.0 {
        (sx * lx).max(MIN_SIZE)
    } else {
        o.width
    };
    let mut h = if sy != 0.0 {
        (sy * ly).max(MIN_SIZE)
    } else {
        o.height
    };
    if keep_aspect && sx != 0.0 && sy != 0.0 {
        let f = (w / o.width).max(h / o.height);
        w = o.width * f;
        h = o.height * f;
    }
    let (ncx, ncy) = rotate_about(ax + sx * w / 2.0, ay + sy * h / 2.0, ax, ay, o.rotation);
    SelectionBox {
        id: o.id.clone(),
        x: ncx - w / 2.0,
        y: ncy - h / 2.0,
        width: w,
        height: h,
        rotation: o.rotation,
    }
}

fn geometry_spec(slide: usize, o: &SelectionBox, l: &SelectionBox, kind: Kind) -> Option<String> {
    let mut spec = json!({
        "command": "patchShapeGeometry",
        "slideIndex": slide,
        "elementId": o.id,
    });
    let m = spec.as_object_mut()?;
    match kind {
        Kind::Move => {
            m.insert("x".into(), l.x.into());
            m.insert("y".into(), l.y.into());
        }
        Kind::Resize(_) => {
            m.insert("x".into(), l.x.into());
            m.insert("y".into(), l.y.into());
            m.insert("width".into(), l.width.into());
            m.insert("height".into(), l.height.into());
        }
        Kind::Rotate => {
            m.insert("rotation".into(), l.rotation.into());
        }
    }
    (o != l).then(|| spec.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_rotated_resize_moving_the_opposite_corner() {
        // Catches: dragging a handle on a rotated shape shifting the anchored opposite edge,
        // so the shape slides across the slide while it is being resized.
        let o = SelectionBox {
            id: "a".into(),
            x: 100.0,
            y: 50.0,
            width: 200.0,
            height: 80.0,
            rotation: 30.0,
        };
        let (cx, cy) = (o.x + 100.0, o.y + 40.0);
        let corner = |b: &SelectionBox, fx: f64, fy: f64| {
            rotate_about(
                b.x + fx * b.width,
                b.y + fy * b.height,
                b.x + b.width / 2.0,
                b.y + b.height / 2.0,
                b.rotation,
            )
        };
        let (tx, ty) = rotate_about(cx + 160.0, cy + 90.0, cx, cy, 30.0);
        let n = resized(&o, "se", tx, ty, false);
        let (a0, a1) = (corner(&o, 0.0, 0.0), corner(&n, 0.0, 0.0));
        assert!((a0.0 - a1.0).abs() < 1e-6 && (a0.1 - a1.1).abs() < 1e-6);
        let (b1x, b1y) = corner(&n, 1.0, 1.0);
        assert!((b1x - tx).abs() < 1e-6 && (b1y - ty).abs() < 1e-6);
    }
}
