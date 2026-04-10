use glam::{Mat4, Quat, Vec3};
use std::{ops::Range};
use std::collections::HashSet;
use wgpu::{Device, Queue};
use wgpu::util::DeviceExt;

use crate::texture;

// Can't be 0
pub const MAX_INSTANCES: usize = 1000;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 4]; 4],
}

impl InstanceRaw {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // Model
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Normal (includes a 4th column for memory alignment purposes)
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 20]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 24]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 28]>() as wgpu::BufferAddress,
                    shader_location: 12,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[derive(Clone, Default)]
pub struct Instance {
    pub position: Vec3,
    pub rotation: Quat,
}

impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: (Mat4::from_translation(self.position) * Mat4::from_quat(self.rotation)).to_cols_array_2d(),
            normal: Mat4::from_quat(self.rotation).to_cols_array_2d(),
        }
    }
}

pub struct ModelBucket {
    pub model: Model,
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
    pub model_buckets: Vec<ModelBucket>,
    pub instances: Vec<Instance>,
    pub raw_instances: Vec<InstanceRaw>,
    pub instance_buffer: wgpu::Buffer,
    pub num_slots: usize,
    pub instances_to_update: HashSet<usize>,
}

impl ModelInstances {
    pub fn new(
        device: &Device,
        models: Vec<Model>,
        num_slots: usize,
    ) -> ModelInstances {
        let instances = vec![Instance::default(); MAX_INSTANCES];

        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
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
        let raw_instances: Vec<InstanceRaw> = instances.iter().map(|instance| instance.to_raw()).collect();
        ModelInstances {model_buckets, instances, raw_instances, instance_buffer, num_slots, instances_to_update}
    }

    // TODO test this
    pub fn get_instances_from_model_id(&self, model_id: usize) -> Option<&[Instance]> {
        self.instances.get(self.model_buckets[model_id].start_index..self.model_buckets[model_id].start_index + self.model_buckets[model_id].current_len)
    }

    pub fn get_mut_instance(&mut self, model_id: usize, instance_id: usize) -> &mut Instance {
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

    pub fn add_instance(&mut self, model_id: usize, instance: Instance) {
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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
}

pub struct Material {
    pub name: String,
    pub diffuse_texture: texture::Texture,
    pub normal_texture: texture::Texture,
    pub bind_group: wgpu::BindGroup,
}

impl Material {
    pub fn new(
        device: &wgpu::Device,
        name: &str,
        diffuse_texture: texture::Texture,
        normal_texture: texture::Texture,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                },
            ],
            label: Some(name),
        });

        Self {
            name: String::from(name),
            diffuse_texture,
            normal_texture,
            bind_group,
        }
    }
}

pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
}

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

pub trait DrawModel<'a> {
    #[allow(unused)]
    fn draw_mesh(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        environment_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        environment_bind_group: &'a wgpu::BindGroup,
    );

    #[allow(unused)]
    fn draw_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        environment_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        environment_bind_group: &'a wgpu::BindGroup,
    );
    #[allow(unused)]
    fn draw_model_instanced_with_material(
        &mut self,
        model: &'a Model,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        environment_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
        environment_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_mesh_instanced(
            mesh,
            material,
            0..1,
            camera_bind_group,
            light_bind_group,
            environment_bind_group,
        );
    }

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
        environment_bind_group: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, &material.bind_group, &[]);
        self.set_bind_group(1, camera_bind_group, &[]);
        self.set_bind_group(2, light_bind_group, &[]);
        self.set_bind_group(3, environment_bind_group, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    fn draw_model(
        &mut self,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
        environment_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_model_instanced(
            model,
            0..1,
            camera_bind_group,
            light_bind_group,
            environment_bind_group,
        );
    }

    fn draw_model_instanced(
        &mut self,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
        environment_bind_group: &'b wgpu::BindGroup, // NEW!
    ) {
        for mesh in &model.meshes {
            let material = &model.materials[mesh.material];
            self.draw_mesh_instanced(
                mesh,
                material,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
                environment_bind_group,
            );
        }
    }

    fn draw_model_instanced_with_material(
        &mut self,
        model: &'b Model,
        material: &'b Material,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
        environment_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            self.draw_mesh_instanced(
                mesh,
                material,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
                environment_bind_group,
            );
        }
    }
}

pub trait Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

impl Vertex for ModelVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Tex Coords
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Normal
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Tangent
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Bitangent
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub trait DrawLight<'a> {
    fn draw_light_mesh(
        &mut self,
        mesh: &'a Mesh,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_light_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_light_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_light_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawLight<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_light_mesh(
        &mut self,
        mesh: &'b Mesh,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_light_mesh_instanced(mesh, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_light_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, camera_bind_group, &[]);
        self.set_bind_group(1, light_bind_group, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    fn draw_light_model(
        &mut self,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_light_model_instanced(model, 0..1, camera_bind_group, light_bind_group);
    }
    fn draw_light_model_instanced(
        &mut self,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            self.draw_light_mesh_instanced(mesh, instances.clone(), camera_bind_group, light_bind_group);
        }
    }
}