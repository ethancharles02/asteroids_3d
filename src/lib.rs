use std::collections::HashSet;
use std::{sync::Arc};
use model::{Vertex, Model, DrawModel, DrawLight};
use glam::{Quat, Vec3, EulerRot};
use wgpu::{Device, Queue};
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};
use web_time::{Instant, Duration};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
mod camera;
mod hdr;
mod model;
mod resources;
mod texture;
mod game;

// Can't be 0
const MAX_INSTANCES: usize = 1000;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniform {
    position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding2: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CursorUniform {
    pos: [f32; 2],
    size: [f32; 2],
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CursorVertex {
    position: [f32; 2],
}

impl CursorVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CursorVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

pub struct ModelBucket {
    model: Model,
    start_index: usize,
    current_len: usize,
    // instance_ids: std::ops::Range<u32>,
}

impl ModelBucket {
    pub fn get_instance_range(&self) -> std::ops::Range<u32> {
        std::ops::Range { start: self.start_index as u32, end: (self.start_index + self.current_len) as u32 }
    }
}

pub struct ModelInstances {
    model_buckets: Vec<ModelBucket>,
    instances: Vec<model::Instance>,
    raw_instances: Vec<model::InstanceRaw>,
    instance_buffer: wgpu::Buffer,
    num_slots: usize,
    instances_to_update: HashSet<usize>,
}

impl ModelInstances {
    pub fn new(
        device: &Device,
        models: Vec<model::Model>,
        num_slots: usize,
    ) -> ModelInstances {
        let instances = vec![model::Instance::default(); MAX_INSTANCES];

        let instance_data = instances.iter().map(model::Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });


        let mut model_buckets: Vec<ModelBucket> = Vec::new();
        for model in models {
            model_buckets.push(ModelBucket { model: model, start_index: 0, current_len: 0 });
        }
        let instances_to_update = HashSet::new();
        let raw_instances: Vec<model::InstanceRaw> = instances.iter().map(|instance| instance.to_raw()).collect();
        ModelInstances {model_buckets, instances, raw_instances, instance_buffer, num_slots, instances_to_update}
    }

    // TODO test this
    pub fn get_instances_from_model_id(&self, model_id: usize) -> Option<&[model::Instance]> {
        self.instances.get(self.model_buckets[model_id].start_index..self.model_buckets[model_id].start_index + self.model_buckets[model_id].current_len)
    }

    pub fn get_mut_instance(&mut self, model_id: usize, instance_id: usize) -> &mut model::Instance {
        if !self.is_index_valid(model_id, instance_id) {panic!("Tried to index outside of current model bucket or model bucket doesn't exist")};

        let global_idx = self.model_buckets[model_id].start_index + instance_id;
        self.instances_to_update.insert(global_idx);
        &mut self.instances[global_idx]
    }

    pub fn update_instance_buffer(&mut self, queue: &Queue) {
        if self.instances_to_update.is_empty() {
            return;
        }

        // Updates any corresponding raw instances
        for &idx in &self.instances_to_update {
            self.raw_instances[idx] = self.instances[idx].to_raw();
        }

        // Writes the whole buffer over. Since most objects are moving in this game, this works. If
        // we had a lot of static objects, it would probably be more optimal to write to specific
        // parts in the buffer
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&self.raw_instances));
    }

    pub fn add_instance(&mut self, model_id: usize, instance: model::Instance) {
        if !self.is_model_index_valid(model_id) {panic!("Model bucket doesn't exist")};

        let mut good_to_add = false;
        if let Some(last_item) = self.model_buckets.last() {
            good_to_add = true;
            if last_item.start_index + last_item.current_len + 1 > self.num_slots {
                // TODO it is outside of scope currently to have the buffer be dynamically expanded.
                // If we want to do it in the future, we need to re-create the buffer and fill the
                // instances/raw instances with data
                // let slots_to_add = self.num_slots
                // This is pretty unnecessary considering the above calculation could possibly
                // overflow
                // if let Some(new_num_slots) = self.num_slots.checked_add(slots_to_add) {
                //     self.num_slots = new_num_slots;
                //     for _ in 0..slots_to_add {
                //         self.instances.push();
                //     }
                //     good_to_add = true;
                // } else {
                //     println!("Failed to allocate more slots for objects")
                // }
                panic!("Couldn't add more objects to the object buffer, buffer is full");
            }
        }

        if good_to_add {
            self.raw_instances.pop();
            self.instances.pop();
            let insert_index = self.model_buckets[model_id].start_index + self.model_buckets[model_id].current_len;
            self.raw_instances.insert(insert_index, instance.to_raw());
            self.instances.insert(insert_index, instance);

            self.model_buckets[model_id].current_len += 1;
            // Add 1 to all later buckets start indices
            if let Some(buckets_to_shift) = self.model_buckets.get_mut((model_id + 1)..) {
                for bucket in buckets_to_shift {
                    bucket.start_index += 1;
                }
            }
        }
    }

    // TODO test this
    pub fn remove_instance(&mut self, model_id: usize, instance_id: usize) {
        if !self.is_index_valid(model_id, instance_id) {panic!("Tried to index outside of current model bucket or model bucket doesn't exist")};
        let global_idx = self.model_buckets[model_id].start_index + instance_id;
        self.instances.remove(global_idx);
        self.raw_instances.remove(global_idx);
        self.model_buckets[model_id].current_len -= 1;
        // Remove 1 from all later buckets start indices
        if let Some(buckets_to_shift) = self.model_buckets.get_mut((model_id + 1)..) {
            for bucket in buckets_to_shift {
                bucket.start_index -= 1;
            }
        }
    }

    fn is_model_index_valid(&self, model_id: usize) -> bool {
        if model_id >= self.model_buckets.len() {
            return false;
        }
        return true;
    }

    fn is_index_valid(&self, model_id: usize, instance_id: usize) -> bool {
        return self.is_model_index_valid(model_id) && instance_id < self.model_buckets[model_id].current_len;
    }
}

// This will store the state of our game
pub struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    model_instances: ModelInstances,
    camera: camera::Camera,
    projection: camera::Projection,
    camera_controller: camera::CameraController,
    camera_uniform: camera::CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    light_model: Model,
    depth_texture: texture::Texture,
    is_surface_configured: bool,
    light_uniform: LightUniform,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    light_render_pipeline: wgpu::RenderPipeline,
    hdr: hdr::HdrPipeline,
    environment_bind_group: wgpu::BindGroup,
    sky_pipeline: wgpu::RenderPipeline,
    cursor_uniform: CursorUniform,
    cursor_vertex_buffer: wgpu::Buffer,
    cursor_uniform_buffer: wgpu::Buffer,
    cursor_bind_group: wgpu::BindGroup,
    cursor_render_pipeline: wgpu::RenderPipeline,
    game_manager: game::GameManager,
}

fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[wgpu::VertexBufferLayout],
    topology: wgpu::PrimitiveTopology,
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: vertex_layouts,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
        })],
        compilation_options: Default::default(),
    }),
        primitive: wgpu::PrimitiveState {
            topology,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));

        let size = window.inner_size();

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::BROWSER_WEBGPU,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result in all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![surface_format.add_srgb_suffix()],
            desired_maximum_frame_latency: 2,
        };

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Normal map
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        // let camera = camera::Camera::new((0.0, 5.0, 10.0), -90.0_f32.to_radians(), -20.0_f32.to_radians(), 0.0);
        let camera = camera::Camera::new(
            (0.0, 5.0, 10.0),
            -90.0_f32.to_radians(),
            -20.0_f32.to_radians(),
            0.0
        );
        let projection =
        camera::Projection::new(config.width, config.height, 45.0_f32.to_radians(), 0.1, 100.0);
        let mut camera_controller = camera::CameraController::new(4.0, 1.0, 2.0, false);
        camera_controller.init_cursor_position(config.width, config.height);

        let mut camera_uniform = camera::CameraUniform::new();
        camera_uniform.update_view_proj(&camera, &projection);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }],
            label: Some("camera_bind_group"),
        });

        let models = game::GameManager::get_models(&device, &queue, &texture_bind_group_layout);
        let mut model_instances = ModelInstances::new(&device, models, MAX_INSTANCES);
        let game_manager = game::GameManager::new(&mut model_instances, 1.0, (2.0, 20.0), 3.0);

        let (vertices, indices) = resources::generate_cube(1.0);
        let light_model = resources::load_model_from_vertices_indices(
            "square",
            vertices,
            indices,
            &device,
        );

        let light_uniform = LightUniform {
            position: [2.0, 4.0, 2.0],
            _padding: 0,
            color: [1.0, 1.0, 1.0],
            _padding2: 0,
        };

        // We'll want to update our lights position, so we use COPY_DST
        let light_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Light VB"),
                contents: bytemuck::cast_slice(&[light_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: None,
            });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
            label: None,
        });

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        let hdr = hdr::HdrPipeline::new(&device, &config);

        let hdr_loader = resources::HdrLoader::new(&device);
        let sky_bytes = resources::load_binary("space.hdr")?;
        let sky_texture = hdr_loader.from_equirectangular_bytes(
            &device,
            &queue,
            &sky_bytes,
            1080,
            Some("Sky Texture"),
        )?;

        let environment_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("environment_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            });

        let environment_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("environment_bind_group"),
            layout: &environment_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sky_texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sky_texture.sampler()),
                },
            ],
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&texture_bind_group_layout),
                    Some(&camera_bind_group_layout),
                    Some(&light_bind_group_layout),
                    Some(&environment_layout),
                ],
                immediate_size: 0,
            });

        let render_pipeline = {
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Normal Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
            };
            create_render_pipeline(
                &device,
                &render_pipeline_layout,
                hdr.format(),
                Some(texture::Texture::DEPTH_FORMAT),
                &[model::ModelVertex::desc(), model::InstanceRaw::desc()],
                wgpu::PrimitiveTopology::TriangleList,
                shader,
            )
        };

        let light_render_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&camera_bind_group_layout),
                    Some(&light_bind_group_layout),
                ],
                immediate_size: 0,
            });
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Light Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("light.wgsl").into()),
            };
            create_render_pipeline(
                &device,
                &layout,
                hdr.format(),
                Some(texture::Texture::DEPTH_FORMAT),
                &[model::ModelVertex::desc()],
                wgpu::PrimitiveTopology::TriangleList,
                shader,
            )
        };
        let sky_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Sky Pipeline Layout"),
                bind_group_layouts: &[Some(&camera_bind_group_layout), Some(&environment_layout)],
                immediate_size: 0,
            });
            let shader = wgpu::include_wgsl!("skybox.wgsl");
            create_render_pipeline(
                &device,
                &layout,
                hdr.format(),
                Some(texture::Texture::DEPTH_FORMAT),
                &[],
                wgpu::PrimitiveTopology::TriangleList,
                shader,
            )
        };

        let cursor_vertices: &[CursorVertex] = &[
            CursorVertex { position: [0.0, 0.0] },
            CursorVertex { position: [1.0, 0.0] },
            CursorVertex { position: [1.0, 1.0] },
            CursorVertex { position: [0.0, 0.0] },
            CursorVertex { position: [1.0, 1.0] },
            CursorVertex { position: [0.0, 1.0] },
        ];

        let cursor_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cursor Vertex Buffer"),
            contents: bytemuck::cast_slice(cursor_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cursor_uniform = CursorUniform {
            pos: [config.width as f32 * 0.5, config.height as f32 * 0.5],
            size: [10.0, 10.0],
            screen_size: [config.width as f32, config.height as f32],
            _padding: [0.0, 0.0],
        };

        let cursor_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cursor Uniform Buffer"),
            contents: bytemuck::cast_slice(&[cursor_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let cursor_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Cursor Bind Group Layout"),
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

        let cursor_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cursor Bind Group"),
            layout: &cursor_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: cursor_uniform_buffer.as_entire_binding(),
            }],
        });

        let cursor_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cursor Pipeline Layout"),
            bind_group_layouts: &[Some(&cursor_bind_group_layout)],
            immediate_size: 0,
        });

        let cursor_render_pipeline = {
            let shader = wgpu::include_wgsl!("cursor.wgsl");
            create_render_pipeline(
                &device,
                &cursor_pipeline_layout,
                config.format,
                None,
                &[CursorVertex::desc()],
                wgpu::PrimitiveTopology::TriangleList,
                shader,
            )
        };

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            render_pipeline,
            model_instances,
            camera,
            projection,
            camera_controller,
            camera_buffer,
            camera_bind_group,
            camera_uniform,
            light_model,
            depth_texture,
            is_surface_configured: false,
            light_uniform,
            light_buffer,
            light_bind_group,
            light_render_pipeline,
            hdr,
            environment_bind_group,
            sky_pipeline,
            cursor_uniform,
            cursor_vertex_buffer,
            cursor_uniform_buffer,
            cursor_bind_group,
            cursor_render_pipeline,
            game_manager,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.projection.resize(width, height);
            self.hdr.resize(&self.device, width, height);
            self.is_surface_configured = true;
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture =
                texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
            self.camera_controller.clamp_cursor_position(width, height);
            self.cursor_uniform.screen_size = [width as f32, height as f32];
            self.update_cursor_uniform();
        }
    }

    fn update_cursor_uniform(&mut self) {
        let (mouse_x, mouse_y) = self.camera_controller.get_cursor_position();
        self.cursor_uniform.pos = [mouse_x, mouse_y];
        self.queue.write_buffer(
            &self.cursor_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.cursor_uniform]),
        );
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, pressed: bool) {
        if !self.game_manager.handle_key(key, pressed, &mut self.camera_controller) {
            match (key, pressed) {
                (KeyCode::Escape, true) => event_loop.exit(),
                _ => {}
            }
        }
    }

    fn handle_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        match button {
            // MouseButton::Left => self.mouse_pressed = pressed,
            _ => {}
        }
    }

    fn handle_mouse_scroll(&mut self, delta: &MouseScrollDelta) {
        self.camera_controller.handle_scroll(delta);
    }

    fn update(&mut self, dt: Duration) {
        // Update camera projection matrix and game objects
        self.game_manager.update(&self.config, dt, &mut self.camera_controller, &mut self.camera, &mut self.model_instances);
        self.camera_uniform.update_view_proj(&self.camera, &self.projection);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        // Update the light
        let old_position = Vec3::from_array(self.light_uniform.position);
        self.light_uniform.position =
            (Quat::from_axis_angle(Vec3::Y, (60.0 * dt.as_secs_f32()).to_radians())
                * old_position)
                .into();
        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );

        self.model_instances.update_instance_buffer(&self.queue);
        self.camera_controller.clamp_cursor_position(self.config.width, self.config.height);
        self.update_cursor_uniform();
    }

    fn render(&mut self) -> anyhow::Result<()> {
        self.window.request_redraw();

        // We can't render unless the surface is configured
        if !self.is_surface_configured {
            return Ok(());
        }

        let output = match self.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                self.surface.configure(&self.device, &self.config);
                surface_texture
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                // Skip this frame
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                // You could recreate the devices and all resources
                // created with it here, but we'll just bail
                anyhow::bail!("Lost device");
            }
        };
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.config.format.add_srgb_suffix()),
                ..Default::default()
            });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.hdr.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_vertex_buffer(1, self.model_instances.instance_buffer.slice(..));
            render_pass.set_pipeline(&self.light_render_pipeline);
            render_pass.draw_light_model(
                &self.light_model,
                &self.camera_bind_group,
                &self.light_bind_group,
            );

            render_pass.set_pipeline(&self.render_pipeline);
            for model_instance_map in &self.model_instances.model_buckets {
                render_pass.draw_model_instanced(
                    &model_instance_map.model,
                    model_instance_map.get_instance_range(),
                    &self.camera_bind_group,
                    &self.light_bind_group,
                    &self.environment_bind_group,
                );
            }

            render_pass.set_pipeline(&self.sky_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.environment_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        self.hdr.process(&mut encoder, &view);

        if !self.camera_controller.is_free() {
            let mut overlay_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Cursor Overlay Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            overlay_pass.set_pipeline(&self.cursor_render_pipeline);
            overlay_pass.set_bind_group(0, &self.cursor_bind_group, &[]);
            overlay_pass.set_vertex_buffer(0, self.cursor_vertex_buffer.slice(..));
            overlay_pass.draw(0..6, 0..1);
        }

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub struct App {
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<State>>,
    state: Option<State>,
    last_time: Instant,
}

impl App {
    pub fn new(#[cfg(target_arch = "wasm32")] event_loop: &EventLoop<State>) -> Self {
        #[cfg(target_arch = "wasm32")]
        let proxy = Some(event_loop.create_proxy());
        Self {
            state: None,
            #[cfg(target_arch = "wasm32")]
            proxy,
            last_time: Instant::now(),
        }
    }
}

impl ApplicationHandler<State> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes();

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;

            const CANVAS_ID: &str = "canvas";

            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();
            let canvas = document.get_element_by_id(CANVAS_ID).unwrap_throw();
            let html_canvas_element = canvas.unchecked_into();
            window_attributes = window_attributes.with_canvas(Some(html_canvas_element));
        }

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.state = Some(pollster::block_on(State::new(window)).unwrap());
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(proxy) = self.proxy.take() {
                wasm_bindgen_futures::spawn_local(async move {
                    assert!(proxy
                        .send_event(
                            State::new(window)
                                .await
                                .expect("Unable to create canvas!!!")
                        )
                        .is_ok())
                });
            }
        }
    }

    #[allow(unused_mut)]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, mut event: State) {
        #[cfg(target_arch = "wasm32")]
        {
            event.window.request_redraw();
            event.resize(
                event.window.inner_size().width,
                event.window.inner_size().height,
            );
        }
        self.state = Some(event);
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        let state = if let Some(state) = &mut self.state {
            state
        } else {
            return;
        };
        match event {
            DeviceEvent::MouseMotion { delta: (dx, dy) } => {
                state.camera_controller.handle_mouse(dx, dy);
            }
            _ => {}
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };

        match event {
            WindowEvent::Focused(true) => {
                // Hide the cursor and locks it
                state.window.set_cursor_visible(false);
                state.window.set_cursor_grab(winit::window::CursorGrabMode::Locked)
                .unwrap_or_else(|e| eprintln!("Failed to grab cursor: {:?}", e));
            }
            WindowEvent::Focused(false) => {
                // Show the cursor and release it
                state.window.set_cursor_visible(true);
                state.window.set_cursor_grab(winit::window::CursorGrabMode::None)
                    .unwrap_or_else(|e| eprintln!("Failed to release cursor: {:?}", e));
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                let dt = self.last_time.elapsed();
                self.last_time = Instant::now();
                state.update(dt);
                match state.render() {
                    Ok(_) => {}
                    Err(e) => {
                        // Log the error and exit gracefully
                        log::error!("{e}");
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: btn_state,
                button,
                ..
            } => state.handle_mouse_button(button, btn_state.is_pressed()),
            WindowEvent::MouseWheel { delta, .. } => {
                state.handle_mouse_scroll(&delta);
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => state.handle_key(event_loop, code, key_state.is_pressed()),
            _ => {}
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
    }
    #[cfg(target_arch = "wasm32")]
    {
        console_log::init_with_level(log::Level::Info).unwrap_throw();
    }

    let event_loop = EventLoop::with_user_event().build()?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = App::new();
        event_loop.run_app(&mut app)?;
    }
    #[cfg(target_arch = "wasm32")]
    {
        let app = App::new(&event_loop);
        event_loop.spawn_app(app);
    }

    Ok(())
}