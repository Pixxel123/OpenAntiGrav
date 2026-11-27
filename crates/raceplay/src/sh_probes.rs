//! The spherical-harmonic ambient an Omega hull is lit from.
//!
//! The circuit's `track.shprobes` ([`oag_vex::shprobes`]) blended at each craft's position, one
//! row of a small table per grid slot: nine rgb coefficients, then `(K, Ks, 1, 0)` with
//! `Ship diffuse brightness` and its `(shadow)` twin. The hull's pixel program evaluates
//! `(Ks + (K - Ks) * shadow) * max(0.01, SH(n))` from its row
//! (`mesh/shaders/shade.wesl`, `sh_hull_ambient`; `docs/ghidra/functions/ps4-omega-eu/hull-tone.md`).
//! **Not a render of our scene**: the original's coefficients are this file's too, so a circuit
//! with no file binds no table and leaves every hull on its constant ambient.
//!
//! The table is bound beside the shadow maps (`mesh_render::ShadowMaps::sh_probes`) and a hull
//! names its row through the same `env_layer` HD's environment probes use.

use std::cell::RefCell;

use oag_gpu::writes::Writes;
use oag_mesh::mesh_render::{SH_PROBE_BYTES, SH_PROBE_ROW, SH_PROBE_ROWS};
use oag_vex::shprobes::{ShProbes, Taps};

/// The two states of `OAG_DEBUG_HULL_SHADE_STATE`, the two knockouts of the hull's shadow term
/// (`docs/ghidra/functions/ps4-omega-eu/hull-tone.md`, "The shadow term, read by knockout"):
/// the sun colour at 0 and one ambient scale at 4, the other at 0. A leaves `K 4, Ks 0`, so the
/// hull draws `4 * shadow * SH`; B leaves `K 0, Ks 4`, so it draws `4 * (1 - shadow) * SH`.
/// The knockout law has no engine-light term (same page and section), so the engine light stays
/// off in both and no light is added to the sum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HullShadeState {
    A,
    B,
}

impl HullShadeState {
    /// The hull's `(K, Ks)` in this state.
    fn weights(self) -> (f32, f32) {
        match self {
            Self::A => (4.0, 0.0),
            Self::B => (0.0, 4.0),
        }
    }
}

/// The state `OAG_DEBUG_HULL_SHADE_STATE` names, read once. Unset, or any other text, is none and
/// leaves the hull alone.
fn hull_shade_state() -> Option<HullShadeState> {
    static STATE: std::sync::OnceLock<Option<HullShadeState>> = std::sync::OnceLock::new();
    *STATE.get_or_init(|| parse_hull_shade_state(std::env::var("OAG_DEBUG_HULL_SHADE_STATE").ok()))
}

/// The value of `OAG_DEBUG_HULL_SHADE_STATE` as a state: `a` or `b`, as typed or trimmed.
fn parse_hull_shade_state(value: Option<String>) -> Option<HullShadeState> {
    match value?.trim() {
        "a" => Some(HullShadeState::A),
        "b" => Some(HullShadeState::B),
        _ => None,
    }
}

/// The light a craft's hull is drawn with: `light` itself, or with the sun colour at 0 where
/// `OAG_DEBUG_HULL_SHADE_STATE` names a state. The circuit's own scene block keeps its light; the
/// zeroed light reaches every block built from the hull's scene, the Shine block (which also
/// draws the circuit's shine passes) included.
#[must_use]
pub(crate) fn hull_light(light: oag_mesh::mesh_render::Light) -> oag_mesh::mesh_render::Light {
    if hull_shade_state().is_some() {
        oag_mesh::mesh_render::Light {
            sun: [0.0; 3],
            ..light
        }
    } else {
        light
    }
}

/// The SPU lights a craft's hull is drawn with: `lights` itself, or none where
/// `OAG_DEBUG_HULL_SHADE_STATE` names a state. The knockout law has no engine-light term
/// (`docs/ghidra/functions/ps4-omega-eu/hull-tone.md`, "The shadow term, read by knockout"), and
/// an added light in both states would pull their ratio toward one half.
#[must_use]
pub(crate) fn hull_spu_lights(
    lights: oag_mesh::mesh_render::SpuLights,
) -> oag_mesh::mesh_render::SpuLights {
    if hull_shade_state().is_some() {
        oag_mesh::mesh_render::SpuLights::none()
    } else {
        lights
    }
}

/// A circuit's probes and the two weights its `.envsettings` authors.
#[derive(Debug, Clone)]
pub struct HullSh {
    pub probes: ShProbes,
    /// `Lighting.Ship diffuse brightness`.
    pub k: f32,
    /// Its `(shadow)` twin, `k` where the file authors none.
    pub ks: f32,
}

const CRAFTS: usize = SH_PROBE_ROWS;
const TEXELS: usize = SH_PROBE_ROW;

impl HullSh {
    /// The table row of the craft at `position`: [`TEXELS`] rgba texels.
    #[must_use]
    pub fn row(&self, position: [f32; 3]) -> [f32; TEXELS * 4] {
        self.row_of(self.probes.locate(position))
    }

    fn row_of(&self, taps: Taps) -> [f32; TEXELS * 4] {
        let coefficients = self.probes.blend(taps);
        let mut row = [0.0; TEXELS * 4];
        for (texel, rgb) in row
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(coefficients.as_chunks::<3>().0)
        {
            texel[..3].copy_from_slice(rgb);
        }
        let (k, ks) = hull_shade_state().map_or((self.k, self.ks), HullShadeState::weights);
        row[(TEXELS - 1) * 4..].copy_from_slice(&[k, ks, 1.0, 0.0]);
        row
    }
}

/// The table on the GPU and what each craft's row was last written from.
#[derive(Debug)]
pub struct ShAmbient {
    hull: HullSh,
    buffer: wgpu::Buffer,
    /// The taps each row holds, so a craft that has not moved between probes writes nothing.
    written: RefCell<[Option<Taps>; CRAFTS]>,
}

impl ShAmbient {
    /// The table, every row unflagged until its craft writes it.
    #[must_use]
    pub fn new(device: &wgpu::Device, hull: HullSh) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("spherical-harmonic ambient"),
            size: SH_PROBE_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            hull,
            buffer,
            written: RefCell::new(Default::default()),
        }
    }

    /// The table, for `ShadowMaps::sh_probes`.
    #[must_use]
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Writes the row of grid slot `craft` for a craft at `position`.
    pub fn write(&self, queue: &dyn Writes, craft: usize, position: [f32; 3]) {
        if craft >= CRAFTS {
            return;
        }
        let taps = self.hull.probes.locate(position);
        if self.written.borrow()[craft] == Some(taps) {
            return;
        }
        let row = self.hull.row_of(taps);
        queue.write_buffer(
            &self.buffer,
            (craft * TEXELS * 16) as u64,
            bytemuck::cast_slice(&row),
        );
        self.written.borrow_mut()[craft] = Some(taps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_shade_state_parses_a_and_b_only() {
        let parse = |text: &str| parse_hull_shade_state(Some(text.to_owned()));
        assert_eq!(parse("a"), Some(HullShadeState::A));
        assert_eq!(parse(" b "), Some(HullShadeState::B));
        assert_eq!(parse("A"), None);
        assert_eq!(parse("ab"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse_hull_shade_state(None), None);
    }
}
