//! The first map a Wipeout HD craft carries: its own silhouette from the sun,
//! as depth, which the hull's sun term is gated by before the road's mask
//! ([`super::occlusion`]) gets its turn.
//!
//! # What the original does
//!
//! `Shadow_RenderModelShadowMaps` (`0x003ed810`) binds each ship's `+0` map as
//! the **depth** target with no colour target, masks colour off, clears depth
//! to far, sets `SET_CULL_FACE` to `GL_FRONT` for the pass and draws the ship
//! through the same sun-view box `Shadow_BuildShipShadowMatrices` fits for
//! the occlusion map. The hull's `ShadowMap` variant then reads it as
//! `directionalLight0ShadowTex` through a projective, hardware-compared tap
//! (`TXP R0.x, f[TC0] unit1`) and multiplies the sun by the result. One
//! caster per map, so a wing shadows the fuselage and nothing shadows
//! another craft. Read on
//! [`ship-sun-occlusion.md`](../../../../docs/ghidra/functions/ps3-hdfury-eu/ship-sun-occlusion.md).
//!
//! # What is the original's and what is ours
//!
//! **The original's**: that the map is depth, rendered colour-masked-off with
//! front faces culled from the occlusion map's own box, with the craft as its
//! only caster, and that the hull compares against it.
//!
//! **Ours**: that every layer is slot 0's 512 texels ([`SIZE`] - the slot
//! assignment is not read), the slope-scaled rasteriser bias and the small constant
//! floor beside it in `mesh`'s shader, and the four-tap comparison - the RSX's
//! own depth-texture compare is one hardware tap whose filtering is unread.
//!
//! # Shape
//!
//! One `Depth32Float` array of [`OCCLUSION_LAYERS`] layers, bound
//! beside the occlusion array in the scene group and read through its
//! non-filtering depth sampler, by the same layer index the hull's uniform
//! names for the occlusion map. Owned by [`super::occlusion::Maps`] rather
//! than by the scene, so the two maps share a matrix by construction: the
//! shader projects one `f[TC0]` through one `directionalLight0Proj` for both
//! taps, and a second `Fit` computed at a second call site could drift from
//! the first while still drawing something that reads as a shadow.

use oag_core::math::Mat4;

use super::layer_uniforms::LayerUniforms;
use super::map::Caster;
use oag_mesh::mesh::{DrawCall, GpuVertex};
use oag_mesh::mesh_render::{OCCLUSION_LAYERS, SELF_SHADOW_FORMAT};

/// Each layer's resolution, square.
///
/// **The original's ratio, not its assignment**: `g_ShipMapSizes` gives every
/// slot a depth map at least as large as its occlusion map, and slot 0's is
/// 512 against 256. Which craft gets slot 0 is unread, so every layer here
/// gets the largest, as [`super::occlusion::SIZE`] does - 8 MiB for the
/// array. Twice the occlusion map's density matters more here than there: a
/// craft covers a few units of a 24-unit box, and a wing's shadow edge is a
/// texel of this map where the road's mask is a texel of the other.
pub const SIZE: u32 = 512;

/// Bytes per layer uniform: one `mat4x4`, padded to the dynamic-offset
/// alignment every backend accepts.
const UNIFORM_STRIDE: u64 = 256;

/// The depth maps, their pipeline and the per-layer uniforms.
#[derive(Debug)]
pub struct Maps {
    texture: wgpu::Texture,
    array_view: wgpu::TextureView,
    layer_views: Vec<wgpu::TextureView>,
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    binds: Vec<wgpu::BindGroup>,
    /// Each layer's matrix, staged for [`Self::write_uniforms`].
    staged: LayerUniforms,
    /// The pipeline [`Self::render_with_circuit`] draws the circuit's pieces
    /// through: the craft's pipeline with no face culling, chosen, not
    /// measured.
    circuit_pipeline: wgpu::RenderPipeline,
    /// The circuit's own matrix per layer: the circuit and the craft have
    /// different model matrices, so one layer needs two.
    circuit_uniforms: wgpu::Buffer,
    circuit_binds: Vec<wgpu::BindGroup>,
    /// Each layer's circuit matrix, staged for [`Self::write_uniforms`].
    circuit_staged: LayerUniforms,
    /// How many index ranges each layer's last pass drew - a cleared layer
    /// and a never-rendered one look identical otherwise.
    drawn: [usize; OCCLUSION_LAYERS as usize],
}

impl Maps {
    /// Builds the array and the pass's pipeline.
    #[must_use]
    pub fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("self shadow maps"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: OCCLUSION_LAYERS,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SELF_SHADOW_FORMAT,
            // `COPY_SRC` so a test can read a layer back, as the occlusion
            // array's is: the only other observable is a compare inside the
            // hull's shader, and a pass that culls the whole craft away
            // still renders a frame.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
            texture_binding_view_dimension: None,
        });
        let array_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("self shadow maps"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layer_views = (0..OCCLUSION_LAYERS)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("self shadow map layer"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();

        // The coverage caster's own vertex stage: position through one
        // matrix, and nothing else is read. Its fragment stage is not bound.
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("self shadow caster"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("OUT_DIR"), "/caster.wgsl")).into(),
            ),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("self shadow caster"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("self shadow caster"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let caster_pipeline = |label: &str, cull_mode: Option<wgpu::Face>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<GpuVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    })],
                    compilation_options: Default::default(),
                },
                // No fragment stage: depth and nothing else, as the original's
                // colour mask of `(0,0,0,0)` with no colour target bound.
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    cull_mode,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: SELF_SHADOW_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    // The slope-scaled bias `super::map::Map::new`'s depth
                    // pipeline carries, for the reason it gives: a panel seen
                    // edge-on from the sun spans many depth units per texel.
                    bias: wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 3.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        // **Front faces culled, as the original's `SET_CULL_FACE` `0x404` for
        // this pass** - and here, unlike the `mapped` tier's depth map, it is
        // the right trade: the caster is the receiver, so the map holds the
        // hull's far side and every sunlit face compares against its own
        // underside rather than against itself. A wing's underside is still
        // nearer the sun than the fuselage below it, which is the shadow that
        // is wanted. Front faces are counter-clockwise, wgpu's default,
        // measured for the main pass in `mesh_render`.
        let pipeline = caster_pipeline("self shadow caster", Some(wgpu::Face::Front));
        // **No culling for the circuit's pieces. Chosen, not measured**: the
        // original's caster program for its circuit-shade pass is unread
        // ([`track-shadow.md`](../../../../docs/ghidra/functions/ps4-omega-eu/track-shadow.md),
        // "The circuit's pieces cast onto craft"). A circuit piece is often a
        // single-sided panel, a deck's underside or a roof seen from either
        // side, which a front-face cull would drop whole.
        let circuit_pipeline = caster_pipeline("circuit shade caster", None);

        let layer_uniforms = |label: &str| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: UNIFORM_STRIDE * u64::from(OCCLUSION_LAYERS),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let binds = (0..OCCLUSION_LAYERS)
                .map(|layer| {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(label),
                        layout: &layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &buffer,
                                offset: UNIFORM_STRIDE * u64::from(layer),
                                size: std::num::NonZeroU64::new(64),
                            }),
                        }],
                    })
                })
                .collect();
            (buffer, binds)
        };
        let (uniforms, binds) = layer_uniforms("self shadow caster uniforms");
        let (circuit_uniforms, circuit_binds) = layer_uniforms("circuit shade caster uniforms");

        Self {
            texture,
            array_view,
            layer_views,
            pipeline,
            uniforms,
            binds,
            staged: LayerUniforms::new(UNIFORM_STRIDE, OCCLUSION_LAYERS),
            circuit_pipeline,
            circuit_uniforms,
            circuit_binds,
            circuit_staged: LayerUniforms::new(UNIFORM_STRIDE, OCCLUSION_LAYERS),
            drawn: [0; OCCLUSION_LAYERS as usize],
        }
    }

    /// Clears `layer` to the far plane and draws `caster` into it through
    /// `matrix` - world to the map's clip space, the occlusion layer's own.
    ///
    /// Returns how many index ranges it drew.
    ///
    /// **Far, every frame**, for the reason the occlusion layer clears black:
    /// a stale depth is last frame's silhouette shadowing a craft that has
    /// turned since.
    ///
    /// The layer's matrix is staged, not written: [`Self::write_uniforms`] writes the
    /// frame's layers in one go, before the submission that draws them.
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        layer: usize,
        matrix: Mat4,
        caster: &Caster<'_>,
    ) -> usize {
        let Some(view) = self.layer_views.get(layer) else {
            return 0;
        };
        self.staged.stage(layer, matrix * caster.model);
        self.drawn[layer] = caster.ranges.len();
        let stamp = oag_gpu::timing::stamp("self shadow");
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("self shadow map"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: stamp.as_ref().map(oag_gpu::timing::Stamp::render_writes),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        oag_gpu::perfprobe::marks::mark(&mut pass, "self shadow");
        if !caster.ranges.is_empty() {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.binds[layer], &[]);
            let mut cursor = caster.geometry.cursor(&mut pass);
            pass.set_index_buffer(caster.indices.slice(..), wgpu::IndexFormat::Uint32);
            for range in caster.ranges {
                cursor.draw(&mut pass, range);
            }
        }
        oag_gpu::perfprobe::marks::close(&mut pass);
        caster.ranges.len()
    }

    /// Clears `layer` to the far plane and draws into it, through `matrix`, the
    /// pieces of `circuit` that pass `near` and then `caster`: Omega's
    /// circuit-shade map, which holds the circuit pieces near a craft and the
    /// craft itself
    /// ([`track-shadow.md`](../../../../docs/ghidra/functions/ps4-omega-eu/track-shadow.md),
    /// "The circuit's pieces cast onto craft").
    ///
    /// Returns how many draws it made: the circuit's draw calls and the
    /// craft's index ranges. It is not zero while the craft has a range,
    /// whether or not a circuit piece is near.
    ///
    /// The circuit's opaque and cutout draws cast, drawn with no face culling.
    /// **A cutout draws solid, chosen, not measured**: the caster has no
    /// fragment stage. The craft's ranges cast through [`Self::render`]'s
    /// pipeline, front faces culled. **The transparent draws do not cast.
    /// Chosen, not measured**: the original clears the caster bits of a piece whose
    /// material binds `screenSpaceRefractionTex`, so a refractive piece does
    /// not cast, and the transparent draws are the nearest set this crate
    /// has to that rule.
    ///
    /// Both matrices are staged, not written, as [`Self::render`]'s is.
    pub fn render_with_circuit(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        layer: usize,
        matrix: Mat4,
        circuit: &super::occlusion::Track<'_>,
        near: impl Fn(&DrawCall) -> bool,
        caster: &Caster<'_>,
    ) -> usize {
        let Some(view) = self.layer_views.get(layer) else {
            return 0;
        };
        self.staged.stage(layer, matrix * caster.model);
        self.circuit_staged.stage(layer, matrix * circuit.model);
        let stamp = oag_gpu::timing::stamp("self shadow");
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("circuit shade map"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: stamp.as_ref().map(oag_gpu::timing::Stamp::render_writes),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        oag_gpu::perfprobe::marks::mark(&mut pass, "self shadow circuit");
        // The pipeline and buffers are set on the first piece within reach, as
        // `super::occlusion::Maps::render` sets them.
        let mut pieces = 0;
        let mut cursor = None;
        for draw in circuit
            .draws
            .iter()
            .chain(circuit.cutouts)
            .filter(|draw| near(draw))
        {
            if pieces == 0 {
                pass.set_pipeline(&self.circuit_pipeline);
                pass.set_bind_group(0, &self.circuit_binds[layer], &[]);
                cursor = Some(circuit.geometry.cursor(&mut pass));
                pass.set_index_buffer(circuit.indices.slice(..), wgpu::IndexFormat::Uint32);
            }
            pieces += 1;
            if let Some(cursor) = &mut cursor {
                cursor.draw(&mut pass, &draw.range);
            }
        }
        if !caster.ranges.is_empty() {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.binds[layer], &[]);
            let mut cursor = caster.geometry.cursor(&mut pass);
            pass.set_index_buffer(caster.indices.slice(..), wgpu::IndexFormat::Uint32);
            for range in caster.ranges {
                cursor.draw(&mut pass, range);
            }
        }
        oag_gpu::perfprobe::marks::close(&mut pass);
        drop(pass);
        let drawn = pieces + caster.ranges.len();
        self.drawn[layer] = drawn;
        drawn
    }

    /// Writes every layer [`Self::render`] and [`Self::render_with_circuit`]
    /// staged since the last call: one write for each buffer.
    pub fn write_uniforms(&mut self, queue: &dyn oag_gpu::writes::Writes) {
        self.staged.write(queue, &self.uniforms);
        self.circuit_staged.write(queue, &self.circuit_uniforms);
    }

    /// Clears `layer` to the far plane without drawing.
    pub fn clear(&mut self, encoder: &mut wgpu::CommandEncoder, layer: usize) {
        let Some(view) = self.layer_views.get(layer) else {
            return;
        };
        self.drawn[layer] = 0;
        let stamp = oag_gpu::timing::stamp("self shadow");
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("self shadow map clear"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: stamp.as_ref().map(oag_gpu::timing::Stamp::render_writes),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        oag_gpu::perfprobe::marks::mark(&mut pass, "self shadow clear");
        oag_gpu::perfprobe::marks::close(&mut pass);
    }

    /// The array view the hull pipelines compare against, through the scene
    /// group.
    #[must_use]
    pub fn view(&self) -> &wgpu::TextureView {
        &self.array_view
    }

    /// How many index ranges `layer`'s last pass drew.
    #[must_use]
    pub fn drawn(&self, layer: usize) -> usize {
        self.drawn.get(layer).copied().unwrap_or(0)
    }

    /// The texture behind [`Self::view`], for a reader that wants a layer
    /// back.
    #[must_use]
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// Reads `layer` back as one `f32` depth per texel, row-major from the
    /// top - [`SIZE`] squared of them - blocking on the GPU. For a headless
    /// check; the frame never calls this.
    #[must_use]
    pub fn read_back(&self, device: &wgpu::Device, queue: &wgpu::Queue, layer: u32) -> Vec<f32> {
        let row = (SIZE * 4).div_ceil(256) * 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("self shadow readback"),
            size: u64::from(row * SIZE),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer.min(OCCLUSION_LAYERS - 1),
                },
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(SIZE),
                },
            },
            wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("draining the queue");
        let view = slice.get_mapped_range().expect("mapping the readback");
        let mut out = vec![0f32; (SIZE * SIZE) as usize];
        for y in 0..SIZE as usize {
            let start = y * row as usize;
            for x in 0..SIZE as usize {
                let at = start + x * 4;
                out[y * SIZE as usize + x] =
                    f32::from_le_bytes([view[at], view[at + 1], view[at + 2], view[at + 3]]);
            }
        }
        drop(view);
        buffer.unmap();
        out
    }
}
